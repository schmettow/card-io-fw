use embedded_graphics::{pixelcolor::BinaryColor, prelude::DrawTarget, Drawable};
use embedded_layout::{
    chain,
    prelude::{Chain, Link},
};
use embedded_menu::{
    collection::MenuItems,
    interaction::single_touch::SingleTouch,
    items::NavigationItem,
    selection_indicator::{style::animated_triangle::AnimatedTriangle, AnimatedPosition},
    Menu, MenuState,
};

use crate::{screens::MENU_STYLE, widgets::status_bar::StatusBar};

#[derive(Clone, Copy)]
pub enum WifiStaMenuEvents {
    None,
    Back,
}

type WifiStaMenu<'a, 'b> = Menu<
    SingleTouch,
    chain! {
        MenuItems<
            'b,
            NavigationItem<'a, WifiStaMenuEvents>,
            WifiStaMenuEvents
        >,
        NavigationItem<'a, WifiStaMenuEvents>
    },
    WifiStaMenuEvents,
    BinaryColor,
    AnimatedPosition,
    AnimatedTriangle,
>;

pub struct WifiStaMenuData<'a, 'b> {
    pub networks: &'b mut [NavigationItem<'a, WifiStaMenuEvents>],
}

impl<'a, 'b> WifiStaMenuData<'a, 'b> {
    pub fn create(
        &'b mut self,
        state: MenuState<SingleTouch, AnimatedPosition, AnimatedTriangle>,
    ) -> WifiStaMenu<'a, 'b> {
        Menu::with_style("Access points", MENU_STYLE)
            .add_items(self.networks)
            .add_item(NavigationItem::new("Back", WifiStaMenuEvents::Back))
            .build_with_state(state)
    }
}

pub struct WifiStaMenuScreen<'a, 'b> {
    pub menu: WifiStaMenu<'a, 'b>,
    pub status_bar: StatusBar,
}

impl Drawable for WifiStaMenuScreen<'_, '_> {
    type Color = BinaryColor;
    type Output = ();

    #[inline]
    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.menu.draw(display)?;
        self.status_bar.draw(display)?;

        Ok(())
    }
}