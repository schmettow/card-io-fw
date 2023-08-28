use crate::widgets::status_bar::StatusBar;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::DrawTarget, Drawable};
use embedded_layout::prelude::{horizontal, vertical, Align};

/// Screen that has a status bar.
pub struct Screen<C>
where
    C: Drawable,
{
    pub content: C,
    pub status_bar: StatusBar,
}

impl<C> Drawable for Screen<C>
where
    C: Drawable<Color = BinaryColor, Output = ()>,
{
    type Color = BinaryColor;
    type Output = ();

    #[inline]
    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.content.draw(display)?;

        self.status_bar
            .align_to(&display.bounding_box(), horizontal::Right, vertical::Top)
            .draw(display)?;

        Ok(())
    }
}