use crate::{
    board::initialized::Board,
    heap::ALLOCATOR,
    states::{AppMenu, TouchInputShaper, MENU_IDLE_DURATION, MIN_FRAME_TIME},
    timeout::Timeout,
    AppState,
};
use embassy_time::Ticker;
use embedded_graphics::prelude::*;
use embedded_menu::{items::NavigationItem, Menu};
use gui::screens::{menu_style, screen::Screen};

#[derive(Clone, Copy)]
pub enum MainMenuEvents {
    Measure,
    Display,
    About,
    WifiSetup,
    WifiListVisible,
    Storage,
    Shutdown,
}

pub async fn main_menu(board: &mut Board) -> AppState {
    let mut exit_timer = Timeout::new(MENU_IDLE_DURATION);
    info!("Free heap: {} bytes", ALLOCATOR.free());

    let builder = Menu::with_style("Main menu", menu_style());

    let mut optional_items = heapless::Vec::<_, 2>::new();

    if board.can_enable_wifi() {
        unwrap!(optional_items
            .push(NavigationItem::new("Wifi setup", MainMenuEvents::WifiSetup))
            .ok());
        unwrap!(optional_items
            .push(NavigationItem::new(
                "Wifi networks",
                MainMenuEvents::WifiListVisible,
            ))
            .ok());
    }

    let mut menu_screen = Screen {
        content: builder
            .add_item(NavigationItem::new("Measure", MainMenuEvents::Measure))
            .add_item(NavigationItem::new(
                "Display settings",
                MainMenuEvents::Display,
            ))
            .add_item(NavigationItem::new(
                "Storage settings",
                MainMenuEvents::Storage,
            ))
            .add_item(NavigationItem::new("Device info", MainMenuEvents::About))
            .add_items(&mut optional_items[..])
            .add_item(NavigationItem::new("Shutdown", MainMenuEvents::Shutdown))
            .build(),

        status_bar: board.status_bar(),
    };

    let mut ticker = Ticker::every(MIN_FRAME_TIME);
    let mut input = TouchInputShaper::new();

    while !exit_timer.is_elapsed() {
        input.update(&mut board.frontend);
        let is_touched = input.is_touched();
        if is_touched {
            exit_timer.reset();
        }

        if let Some(event) = menu_screen.content.interact(is_touched) {
            match event {
                MainMenuEvents::Measure => return AppState::Initialize,
                MainMenuEvents::Display => return AppState::Menu(AppMenu::Display),
                MainMenuEvents::About => return AppState::Menu(AppMenu::DeviceInfo),
                MainMenuEvents::WifiSetup => return AppState::Menu(AppMenu::WifiAP),
                MainMenuEvents::WifiListVisible => return AppState::Menu(AppMenu::WifiListVisible),
                MainMenuEvents::Storage => return AppState::Menu(AppMenu::Storage),
                MainMenuEvents::Shutdown => return AppState::Shutdown,
            };
        }

        #[cfg(feature = "battery_max17055")]
        if board.battery_monitor.is_low() {
            return AppState::Shutdown;
        }

        menu_screen.status_bar = board.status_bar();

        board
            .display
            .frame(|display| {
                menu_screen.content.update(display);
                menu_screen.draw(display)
            })
            .await;

        ticker.next().await;
    }

    info!("Menu timeout");
    AppState::Shutdown
}
