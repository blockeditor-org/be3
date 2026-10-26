use beui_view::types::SystemBars;
use beui_macros::{component, view};
use beui_view::primitives;
use beui_view::reactive::{Child, Get, Prop, create_memo};

use crate::frame::ColorRole;
use crate::theme::{ThemeKind, use_theme};

pub type Root = primitives::Root;

#[component]
pub fn Root(
    clear_color: Option<Prop<ColorRole>>,
    title: Option<Prop<String>>,
    system_bars: Option<Prop<SystemBars>>,
    children: Child,
) -> primitives::Root {
    let theme = use_theme();
    let color = create_memo(move || {
        clear_color
            .map_or(ColorRole::Background, |role| role.get())
            .resolve(&theme)
    });
    let bars = create_memo(move || match system_bars {
        Some(bars) => bars.get(),
        None => match theme.kind.get() {
            ThemeKind::Light | ThemeKind::EPaper => SystemBars::Light,
            ThemeKind::Dark | ThemeKind::OledDark => SystemBars::Dark,
        },
    });
    match title {
        Some(title) => view! {
            <primitives::Root clear_color={color} title system_bars={bars}>
                {children}
            </primitives::Root>
        },
        None => view! {
            <primitives::Root clear_color={color} system_bars={bars}>{children}</primitives::Root>
        },
    }
}
