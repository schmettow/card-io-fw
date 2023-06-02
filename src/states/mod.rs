mod adc_setup;
mod charging;
mod error;
mod init;
mod measure;
mod menu;
mod wifi_ap;

use embassy_time::Duration;

pub use adc_setup::adc_setup;
pub use charging::charging;
pub use error::app_error;
pub use init::initialize;
pub use measure::measure;
pub use menu::display::display_menu;
pub use menu::main::main_menu;
pub use wifi_ap::wifi_ap;

const MIN_FRAME_TIME: Duration = Duration::from_hz(100);
