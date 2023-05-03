use display_interface::{AsyncWriteOnlyDataCommand, DisplayError};
use embassy_time::Delay;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions, Size},
    Pixel,
};
use embedded_hal::digital::OutputPin;
use ssd1306::{
    command::AddrMode, mode::BufferedGraphicsMode, rotation::DisplayRotation,
    size::DisplaySize128x64, Ssd1306,
};

pub struct Display<DI, RESET> {
    display: Ssd1306<DI, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    reset: RESET,
}

impl<DI, RESET> Display<DI, RESET>
where
    RESET: OutputPin,
{
    pub fn new(spi: DI, reset: RESET) -> Self {
        Self {
            display: Ssd1306::new(spi, DisplaySize128x64, DisplayRotation::Rotate0)
                .into_buffered_graphics_mode(),
            reset,
        }
    }

    pub async fn enable(&mut self) -> Result<PoweredDisplay<'_, DI, RESET>, DisplayError>
    where
        DI: AsyncWriteOnlyDataCommand,
    {
        self.display
            .reset_async::<_, Delay>(&mut self.reset, &mut Delay)
            .await
            .unwrap();

        self.display
            .init_with_addr_mode_async(AddrMode::Page)
            .await?;

        Ok(PoweredDisplay { display: self })
    }
}

pub struct PoweredDisplay<'a, S, RESET>
where
    RESET: OutputPin,
{
    display: &'a mut Display<S, RESET>,
}

impl<'a, S, RESET> OriginDimensions for PoweredDisplay<'a, S, RESET>
where
    RESET: OutputPin,
{
    fn size(&self) -> Size {
        self.display.display.size()
    }
}

impl<'a, S, RESET> DrawTarget for PoweredDisplay<'a, S, RESET>
where
    RESET: OutputPin,
{
    type Color = BinaryColor;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.display.display.draw_iter(pixels)
    }
}

impl<'a, S, RESET> PoweredDisplay<'a, S, RESET>
where
    RESET: OutputPin,
    S: AsyncWriteOnlyDataCommand,
{
    pub fn clear(&mut self) {
        self.display.display.clear()
    }

    pub async fn flush(&mut self) -> Result<(), DisplayError> {
        self.display.display.flush_async().await
    }

    pub fn shut_down(self) {
        // Implemented in Drop
    }
}

impl<'a, S, RESET> Drop for PoweredDisplay<'a, S, RESET>
where
    RESET: OutputPin,
{
    fn drop(&mut self) {
        self.display.reset.set_low().unwrap();
    }
}
