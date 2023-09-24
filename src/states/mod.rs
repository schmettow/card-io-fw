mod adc_setup;
mod charging;
mod display_serial;
mod init;
mod measure;
mod menu;
mod upload_or_store_measurement;

use embassy_time::{Duration, Timer};
use embedded_graphics::Drawable;

use gui::{
    screens::{message::MessageScreen, screen::Screen},
    widgets::{
        battery_small::Battery,
        status_bar::StatusBar,
        wifi::{WifiState, WifiStateView},
    },
};

pub use adc_setup::adc_setup;
pub use charging::charging;
pub use display_serial::display_serial;
pub use init::initialize;
pub use measure::{measure, ECG_BUFFER_SIZE};
#[cfg(feature = "battery_max17055")]
pub use menu::battery_info::battery_info_menu;
pub use menu::{
    about::about_menu, display::display_menu, main::main_menu, storage::storage_menu,
    wifi_ap::wifi_ap, wifi_sta::wifi_sta, AppMenu,
};
pub use upload_or_store_measurement::{upload_or_store_measurement, upload_stored_measurements};

const TARGET_FPS: u32 = 100;
const MIN_FRAME_TIME: Duration = Duration::from_hz(TARGET_FPS as u64);

const MENU_IDLE_DURATION: Duration = Duration::from_secs(30);

// The max number of webserver tasks.
const WEBSERVER_TASKS: usize = 2;

use signal_processing::lerp::interpolate;

use crate::board::{initialized::Board, wifi::GenericConnectionState, EcgFrontend};

/// Simple utility to process touch events in an interactive menu.
pub struct TouchInputShaper {
    released: bool,
    touched: bool,
}

impl TouchInputShaper {
    pub fn new() -> Self {
        Self {
            released: false,
            touched: false,
        }
    }

    pub fn update(&mut self, frontend: &mut EcgFrontend) {
        self.touched = frontend.is_touched();

        if !self.touched {
            self.released = true;
        }
    }

    pub fn is_touched(&mut self) -> bool {
        self.released && self.touched
    }
}

fn to_progress(elapsed: Duration, max_duration: Duration) -> u32 {
    interpolate(
        elapsed.as_millis() as u32,
        0,
        max_duration.as_millis() as u32,
        0,
        255,
    )
}

async fn display_message(board: &mut Board, message: &str) {
    info!("Displaying message: {}", message);
    board.message_display_timer = Timer::after(Duration::from_secs(2));

    let status_bar = board.status_bar();
    board
        .display
        .frame(|display| {
            Screen {
                content: MessageScreen { message },
                status_bar,
            }
            .draw(display)
        })
        .await;
}

async fn display_message_while_touched(board: &mut Board, message: &str) {
    let mut ticker = embassy_time::Ticker::every(MIN_FRAME_TIME);
    while board.frontend.is_touched() && !board.battery_monitor.is_low() {
        display_message(board, message).await;
        ticker.next().await;
    }
}

impl Board {
    pub fn status_bar(&mut self) -> StatusBar {
        let battery_data = self.battery_monitor.battery_data();
        let connection_state = match self.wifi.connection_state() {
            GenericConnectionState::Sta(state) => Some(WifiState::from(state)),
            GenericConnectionState::Ap(state) => Some(WifiState::from(state)),
            GenericConnectionState::Disabled => None,
        };

        StatusBar {
            battery: Battery::with_style(battery_data, self.config.battery_style()),
            wifi: WifiStateView::new(connection_state),
        }
    }
}
