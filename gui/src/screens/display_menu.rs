use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, Point},
    Drawable,
};
use embedded_layout::prelude::{horizontal, vertical, Align};
use embedded_menu::{
    interaction::single_touch::SingleTouch,
    items::select::SelectValue,
    selection_indicator::{style::animated_triangle::AnimatedTriangle, AnimatedPosition},
    Menu, SelectValue,
};
use norfs::{
    medium::StorageMedium,
    reader::BoundReader,
    storable::{LoadError, Loadable, Storable},
    writer::BoundWriter,
    StorageError,
};

use crate::{
    screens::BatteryInfo,
    widgets::battery_small::{Battery, BatteryStyle},
};

#[derive(Clone, Copy)]
pub enum DisplayMenuEvents {
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, SelectValue)]
pub enum DisplayBrightness {
    Dimmest,
    Dim,
    Normal,
    Bright,
    Brightest,
}

impl Loadable for DisplayBrightness {
    async fn load<M>(reader: &mut BoundReader<'_, M>) -> Result<Self, LoadError>
    where
        M: StorageMedium,
        [(); M::BLOCK_COUNT]: Sized,
    {
        let data = match u8::load(reader).await? {
            0 => Self::Dimmest,
            1 => Self::Dim,
            2 => Self::Normal,
            3 => Self::Bright,
            4 => Self::Brightest,
            _ => return Err(LoadError::InvalidValue),
        };

        Ok(data)
    }
}

impl Storable for DisplayBrightness {
    async fn store<M>(&self, writer: &mut BoundWriter<'_, M>) -> Result<(), StorageError>
    where
        M: StorageMedium,
        [(); M::BLOCK_COUNT]: Sized,
    {
        (*self as u8).store(writer).await
    }
}

impl SelectValue for BatteryStyle {
    fn next(&self) -> Self {
        match self {
            Self::MilliVolts => Self::Percentage,
            Self::Percentage => Self::Icon,
            Self::Icon => Self::LowIndicator,
            Self::LowIndicator => Self::MilliVolts,
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Self::MilliVolts => "MilliVolts",
            Self::Percentage => "Percentage",
            Self::Icon => "Icon",
            Self::LowIndicator => "Indicator",
        }
    }
}

impl Loadable for BatteryStyle {
    async fn load<M>(reader: &mut BoundReader<'_, M>) -> Result<Self, LoadError>
    where
        M: StorageMedium,
        [(); M::BLOCK_COUNT]: Sized,
    {
        let data = match u8::load(reader).await? {
            0 => Self::MilliVolts,
            1 => Self::Percentage,
            2 => Self::Icon,
            3 => Self::LowIndicator,
            _ => return Err(LoadError::InvalidValue),
        };

        Ok(data)
    }
}

impl Storable for BatteryStyle {
    async fn store<M>(&self, writer: &mut BoundWriter<'_, M>) -> Result<(), StorageError>
    where
        M: StorageMedium,
        [(); M::BLOCK_COUNT]: Sized,
    {
        (*self as u8).store(writer).await
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Menu)]
#[menu(
    title = "Display",
    navigation(events = DisplayMenuEvents),
    items = [
        data(label = "Brightness", field = brightness),
        data(label = "Battery", field = battery_display),
        navigation(label = "Back", event = DisplayMenuEvents::Back)
    ]
)]
pub struct DisplayMenu {
    pub brightness: DisplayBrightness,
    pub battery_display: BatteryStyle,
}

pub struct DisplayMenuScreen {
    pub menu: DisplayMenuMenuWrapper<SingleTouch, AnimatedPosition, AnimatedTriangle>,
    pub battery_data: Option<BatteryInfo>,
    pub battery_style: BatteryStyle,
}

impl Drawable for DisplayMenuScreen {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        if let Some(data) = self.battery_data {
            Battery {
                data,
                style: self.battery_style,
                top_left: Point::zero(),
            }
            .align_to(&display.bounding_box(), horizontal::Right, vertical::Top)
            .draw(display)?;
        }

        self.menu.draw(display)
    }
}
