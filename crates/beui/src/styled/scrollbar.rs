use beui_macros::{component, view};

use crate::color::Color32;

use crate::base::{Direction, ScrollPosition};
use crate::node::NodeId;
use crate::reactive::{Frame, ItemSize, List, Prop, Spacer, clone, create_memo};
use crate::styled::theme::{ThemeStore, use_theme};

const RADIUS: u8 = 3;
const MINIMUM_THUMB: f32 = 0.08;

#[component]
pub fn Scrollbar(
    position: Prop<ScrollPosition>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
) -> NodeId {
    let position = create_memo(move || position.get());
    let theme = use_theme();

    let before = create_memo(clone!(position -> move || before_percent(position.get())));
    let thumb = create_memo(clone!(position -> move || thumb_percent(position.get())));
    let after = create_memo(clone!(position -> move || after_percent(position.get())));
    let color = create_memo(clone!(theme -> move || thumb_color(&theme, position.get())));

    view! {
        <Frame color={theme.surface_raised.clone()} radius=RADIUS>
            <List direction spacing=0.0>
                <Spacer @sizing={before} />
                <Frame @sizing={thumb} color radius=RADIUS></Frame>
                <Spacer @sizing={after} />
            </List>
        </Frame>
    }
}

fn visible_fraction(position: ScrollPosition) -> f32 {
    if position.content > 0.0 {
        (position.viewport / position.content).clamp(MINIMUM_THUMB, 1.0)
    } else {
        1.0
    }
}

fn progress_fraction(position: ScrollPosition) -> f32 {
    if position.max_offset() > 0.0 {
        position.offset / position.max_offset()
    } else {
        0.0
    }
}

fn before_percent(position: ScrollPosition) -> ItemSize {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    ItemSize::Percent(rest * progress_fraction(position))
}

fn thumb_percent(position: ScrollPosition) -> ItemSize {
    ItemSize::Percent(visible_fraction(position) * 100.0)
}

fn after_percent(position: ScrollPosition) -> ItemSize {
    let rest = 100.0 - visible_fraction(position) * 100.0;
    ItemSize::Percent(rest * (1.0 - progress_fraction(position)))
}

fn thumb_color(theme: &ThemeStore, position: ScrollPosition) -> Color32 {
    if position.max_offset() > 0.0 {
        theme.scroll_thumb.get()
    } else {
        Color32::TRANSPARENT
    }
}
