use core::{alloc::AllocError, ptr::addr_of, sync::atomic::Ordering};

use crate::{
    board::{
        hal::{radio::Wifi, Rng},
        initialized::Board,
        wifi::net_task,
    },
    states::display_message,
    task_control::{TaskControlToken, TaskController},
    timeout::Timeout,
    Shared,
};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use config_site::data::network::WifiNetwork;
use embassy_executor::Spawner;
use embassy_futures::{
    join::join,
    select::{select, Either},
};
use embassy_net::{dns::DnsSocket, Config, Stack, StackResources};
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    mutex::{Mutex, MutexGuard},
    signal::Signal,
};
use embassy_time::{Duration, Ticker, Timer};
use embedded_svc::wifi::{AccessPointInfo, ClientConfiguration, Configuration, Wifi as _};
use esp_wifi::{
    wifi::{WifiController, WifiDevice, WifiEvent, WifiMode},
    EspWifiInitialization,
};
use gui::widgets::wifi::WifiState;
use macros as cardio;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};

const SCAN_RESULTS: usize = 20;

struct State {
    signal: Signal<NoopRawMutex, ()>,
    value: AtomicInternalConnectionState,
}

impl State {
    fn new(state: InternalConnectionState) -> State {
        Self {
            signal: Signal::new(),
            value: AtomicInternalConnectionState::new(state),
        }
    }

    async fn wait(&self) -> InternalConnectionState {
        self.signal.wait().await;
        self.read()
    }

    fn read(&self) -> InternalConnectionState {
        self.value.load(Ordering::Acquire)
    }

    fn update(&self, value: InternalConnectionState) {
        debug!("Updating connection state: {:?}", value);
        self.value.store(value, Ordering::Release);
        self.signal.signal(());
    }

    fn reset(&self) {
        self.value
            .store(InternalConnectionState::NotConnected, Ordering::Release);
        self.signal.reset();
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum NetworkPreference {
    Preferred,
    Deprioritized,
}

/// A network SSID and password, with an object used to deprioritize unstable networks.
type KnownNetwork = (WifiNetwork, NetworkPreference);

#[derive(PartialEq)]
pub enum ConnectionState {
    NotConnected,
    Connecting,
    Connected,
}

impl From<ConnectionState> for WifiState {
    fn from(state: ConnectionState) -> Self {
        match state {
            ConnectionState::NotConnected => WifiState::NotConnected,
            ConnectionState::Connecting => WifiState::Connecting,
            ConnectionState::Connected => WifiState::Connected,
        }
    }
}

#[derive(PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[atomic_enum::atomic_enum]
enum InternalConnectionState {
    NotConnected,
    Connecting,
    WaitingForIp,
    Connected,
    Disconnected,
}

impl From<InternalConnectionState> for ConnectionState {
    fn from(value: InternalConnectionState) -> Self {
        match value {
            InternalConnectionState::NotConnected | InternalConnectionState::Disconnected => {
                ConnectionState::NotConnected
            }
            InternalConnectionState::Connecting | InternalConnectionState::WaitingForIp => {
                ConnectionState::Connecting
            }
            InternalConnectionState::Connected => ConnectionState::Connected,
        }
    }
}

#[derive(Clone)]
pub struct Sta {
    stack: Rc<Stack<WifiDevice<'static>>>,
    networks: Shared<heapless::Vec<AccessPointInfo, SCAN_RESULTS>>,
    known_networks: Shared<Vec<KnownNetwork>>,
    state: Rc<State>,
    rng: Rng,
}

impl Sta {
    pub fn connection_state(&self) -> ConnectionState {
        self.state.read().into()
    }

    pub async fn visible_networks(
        &self,
    ) -> MutexGuard<'_, NoopRawMutex, heapless::Vec<AccessPointInfo, SCAN_RESULTS>> {
        self.networks.lock().await
    }

    pub async fn update_known_networks(&self, networks: &[WifiNetwork]) {
        let mut known = self.known_networks.lock().await;

        known.retain(|(network, _)| networks.contains(network));
        for network in networks {
            if !known.iter().any(|(kn, _)| kn == network) {
                known.push((network.clone(), NetworkPreference::Deprioritized));
            }
        }
    }

    pub async fn wait_for_state_change(&self) -> ConnectionState {
        self.state.wait().await.into()
    }

    pub async fn wait_for_connection(&self, board: &mut Board) -> bool {
        if self.connection_state() != ConnectionState::Connected {
            debug!("Waiting for network connection");

            let _ = select(
                async {
                    loop {
                        let result =
                            Timeout::with(Duration::from_secs(10), self.wait_for_state_change())
                                .await;
                        match result {
                            Some(ConnectionState::Connected) => break,
                            Some(_state) => {}
                            _ => {
                                debug!("State change timeout");
                                break;
                            }
                        }
                    }
                },
                async {
                    let mut ticker = Ticker::every(Duration::from_millis(100));
                    loop {
                        display_message(board, "Connecting...").await;
                        ticker.next().await;
                    }
                },
            )
            .await;
        }

        if self.connection_state() == ConnectionState::Connected {
            true
        } else {
            debug!("No network connection");
            false
        }
    }

    pub fn stack(&self) -> &Stack<WifiDevice<'static>> {
        &self.stack
    }

    /// Allocates resources for an HTTPS capable [`HttpClient`].
    pub fn https_client_resources(&self) -> Result<HttpsClientResources<'_>, AllocError> {
        // The client state must be heap allocated, because we take a reference to it.
        let resources = Box::try_new(TlsClientState {
            tcp_state: TcpClientState::new(),
            tls_read_buffer: [0; TLS_READ_BUFFER],
            tls_write_buffer: [0; TLS_WRITE_BUFFER],
        })?;
        let client_state = unsafe { addr_of!(resources.tcp_state).as_ref().unwrap() };

        Ok(HttpsClientResources {
            resources,
            tcp_client: TcpClient::new(&self.stack, client_state),
            dns_client: DnsSocket::new(&self.stack),
            rng: self.rng,
        })
    }
}

const SOCKET_COUNT: usize = 1;
const SOCKET_TX_BUFFER: usize = 4096;
const SOCKET_RX_BUFFER: usize = 4096;

const TLS_READ_BUFFER: usize = 16 * 1024 + 256;
const TLS_WRITE_BUFFER: usize = 4096;

type TcpClientState =
    embassy_net::tcp::client::TcpClientState<SOCKET_COUNT, SOCKET_TX_BUFFER, SOCKET_RX_BUFFER>;
type TcpClient<'a> = embassy_net::tcp::client::TcpClient<
    'a,
    WifiDevice<'static>,
    SOCKET_COUNT,
    SOCKET_TX_BUFFER,
    SOCKET_RX_BUFFER,
>;

struct TlsClientState {
    tcp_state: TcpClientState,
    tls_read_buffer: [u8; TLS_READ_BUFFER], // must be 16K
    tls_write_buffer: [u8; TLS_WRITE_BUFFER],
}

pub struct HttpsClientResources<'a> {
    resources: Box<TlsClientState>,
    tcp_client: TcpClient<'a>,
    dns_client: DnsSocket<'a, WifiDevice<'static>>,
    rng: Rng,
}

impl<'a> HttpsClientResources<'a> {
    pub fn client(&mut self) -> HttpClient<'_, TcpClient<'a>, DnsSocket<'a, WifiDevice<'static>>> {
        let upper = self.rng.random() as u64;
        let lower = self.rng.random() as u64;
        let seed = (upper << 32) | lower;

        HttpClient::new_with_tls(
            &self.tcp_client,
            &self.dns_client,
            TlsConfig::new(
                seed,
                &mut self.resources.tls_read_buffer,
                &mut self.resources.tls_write_buffer,
                TlsVerify::None,
            ),
        )
    }
}

pub(super) struct StaState {
    init: EspWifiInitialization,
    controller: Option<Box<WifiController<'static>>>,
    stack: Rc<Stack<WifiDevice<'static>>>,
    networks: Shared<heapless::Vec<AccessPointInfo, SCAN_RESULTS>>,
    known_networks: Shared<Vec<KnownNetwork>>,
    state: Rc<State>,
    connection_task_control: Option<TaskController<(), StaTaskResources>>,
    net_task_control: TaskController<!>,
    rng: Rng,
}

impl StaState {
    pub(super) fn init(
        init: EspWifiInitialization,
        config: Config,
        wifi: &'static mut Wifi,
        resources: &'static mut StackResources<3>,
        mut rng: Rng,
    ) -> Self {
        info!("Configuring STA");

        let (wifi_interface, controller) =
            unwrap!(esp_wifi::wifi::new_with_mode(&init, wifi, WifiMode::Sta));

        let lower = rng.random() as u64;
        let upper = rng.random() as u64;

        let random_seed = upper << 32 | lower;

        Self {
            init,
            controller: Some(Box::new(controller)),
            stack: Rc::new(Stack::new(wifi_interface, config, resources, random_seed)),
            networks: Rc::new(Mutex::new(heapless::Vec::new())),
            known_networks: Rc::new(Mutex::new(Vec::new())),
            state: Rc::new(State::new(InternalConnectionState::NotConnected)),
            connection_task_control: None,
            net_task_control: TaskController::new(),
            rng,
        }
    }

    pub(super) fn unwrap(self) -> EspWifiInitialization {
        self.init
    }

    pub(super) async fn stop(&mut self) {
        if let Some(task_control) = self.connection_task_control.take() {
            info!("Stopping STA");

            let _ = join(task_control.stop(), self.net_task_control.stop()).await;

            let mut controller = task_control.unwrap().controller;
            if matches!(controller.is_started(), Ok(true)) {
                unwrap!(controller.stop().await);
            }

            info!("Stopped STA");

            self.controller = Some(controller);
        }
    }

    pub(super) async fn start(&mut self) -> Sta {
        if let Some(controller) = self.controller.take() {
            info!("Starting STA");
            let spawner = Spawner::for_current_executor().await;

            self.state.reset();

            let task_control = TaskController::from_resources(StaTaskResources { controller });

            info!("Starting STA task");
            spawner.must_spawn(sta_task(
                self.networks.clone(),
                self.known_networks.clone(),
                self.state.clone(),
                self.stack.clone(),
                task_control.token(),
            ));
            info!("Starting NET task");
            spawner.must_spawn(net_task(self.stack.clone(), self.net_task_control.token()));

            self.connection_task_control = Some(task_control);
        }

        self.handle_unchecked()
    }

    pub(crate) fn handle(&self) -> Option<Sta> {
        self.connection_task_control
            .as_ref()
            .map(|_| self.handle_unchecked())
    }

    fn handle_unchecked(&self) -> Sta {
        Sta {
            stack: self.stack.clone(),
            networks: self.networks.clone(),
            known_networks: self.known_networks.clone(),
            state: self.state.clone(),
            rng: self.rng,
        }
    }
}

struct StaTaskResources {
    controller: Box<WifiController<'static>>,
}

#[cardio::task]
async fn sta_task(
    networks: Shared<heapless::Vec<AccessPointInfo, SCAN_RESULTS>>,
    known_networks: Shared<Vec<KnownNetwork>>,
    state: Rc<State>,
    stack: Rc<Stack<WifiDevice<'static>>>,
    mut task_control: TaskControlToken<(), StaTaskResources>,
) {
    const SCAN_PERIOD: Duration = Duration::from_secs(5);
    const CONNECT_RETRY_PERIOD: Duration = Duration::from_millis(100);
    const CONNECT_RETRY_COUNT: usize = 5;

    task_control
        .run_cancellable(|resources| async {
            let controller = &mut resources.controller;

            'scan_and_connect: loop {
                if !matches!(controller.is_started(), Ok(true)) {
                    info!("Starting wifi");
                    unwrap!(controller.start().await);
                    info!("Wifi started!");
                }

                let connect_to = 'select: loop {
                    info!("Scanning...");

                    let mut scan_results = Box::new(controller.scan_n::<SCAN_RESULTS>().await);

                    match scan_results.as_mut() {
                        Ok((ref mut visible_networks, network_count)) => {
                            info!("Found {} access points", network_count);

                            // Sort by signal strength, descending
                            visible_networks
                                .sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));

                            networks.lock().await.clone_from(visible_networks);

                            let mut known_networks = known_networks.lock().await;

                            // Try to find a preferred network.
                            if let Some(connect_to) = select_visible_known_network(
                                &known_networks,
                                visible_networks.as_slice(),
                                NetworkPreference::Preferred,
                            ) {
                                break 'select connect_to.clone();
                            }

                            // No preferred networks in range. Try the naughty list.
                            if let Some(connect_to) = select_visible_known_network(
                                &known_networks,
                                visible_networks.as_slice(),
                                NetworkPreference::Deprioritized,
                            ) {
                                break 'select connect_to.clone();
                            }

                            // No visible known networks. Reset deprioritized networks.
                            for (_, preference) in known_networks.iter_mut() {
                                *preference = NetworkPreference::Preferred;
                            }
                        }
                        Err(err) => warn!("Scan failed: {:?}", err),
                    }

                    Timer::after(SCAN_PERIOD).await;
                };

                info!("Connecting to {}...", connect_to.ssid);
                state.update(InternalConnectionState::Connecting);

                unwrap!(controller.set_configuration(&Configuration::Client(
                    ClientConfiguration {
                        ssid: connect_to.ssid.clone(),
                        password: connect_to.pass,
                        ..Default::default()
                    }
                )));

                for _ in 0..CONNECT_RETRY_COUNT {
                    match controller.connect().await {
                        Ok(_) => {
                            state.update(InternalConnectionState::WaitingForIp);
                            info!("Waiting to get IP address...");

                            let wait_for_ip = async {
                                loop {
                                    if let Some(config) = stack.config_v4() {
                                        info!("Got IP: {}", config.address);
                                        break;
                                    }
                                    Timer::after(Duration::from_millis(500)).await;
                                }
                            };

                            let wait_for_disconnect = async {
                                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                            };

                            match select(wait_for_ip, wait_for_disconnect).await {
                                Either::First(_) => {
                                    info!("Wifi connected!");
                                    state.update(InternalConnectionState::Connected);

                                    // keep pending Disconnected event to avoid a race condition
                                    controller
                                        .wait_for_events(WifiEvent::StaDisconnected.into(), false)
                                        .await;

                                    // TODO: figure out if we should deprioritize, retry or just loop back
                                    // to the beginning. Maybe we could use a timer?
                                    info!("Wifi disconnected!");
                                    state.update(InternalConnectionState::Disconnected);
                                    continue 'scan_and_connect;
                                }
                                Either::Second(_) => {
                                    info!("Wifi disconnected!");
                                    state.update(InternalConnectionState::Disconnected);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to connect to wifi: {:?}", e);
                            state.update(InternalConnectionState::NotConnected);
                            Timer::after(CONNECT_RETRY_PERIOD).await;
                        }
                    }
                }

                // If we get here, we failed to connect to the network. Deprioritize it.
                let mut known_networks = known_networks.lock().await;
                if let Some((_, preference)) = known_networks
                    .iter_mut()
                    .find(|(kn, _)| kn.ssid == connect_to.ssid)
                {
                    *preference = NetworkPreference::Deprioritized;
                }
            }
        })
        .await;
}

fn select_visible_known_network<'a>(
    known_networks: &'a [KnownNetwork],
    visible_networks: &[AccessPointInfo],
    preference: NetworkPreference,
) -> Option<&'a WifiNetwork> {
    for network in visible_networks {
        if let Some((known_network, _)) = known_networks
            .iter()
            .find(|(kn, pref)| kn.ssid == network.ssid && *pref == preference)
        {
            return Some(known_network);
        }
    }

    None
}
