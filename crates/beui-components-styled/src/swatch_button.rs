use beui_components_unstyled as unstyled;
use beui_view::types::Color32;
use beui_view::reactive::NodeId;
use beui_macros::{component, view};
use beui_view::reactive::*;

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::tokens::{Size, Space, Stroke};
use crate::tooltip::Tooltip;

#[component]
pub fn SwatchButton(
    color: Prop<Color32>,
    label: Prop<String>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = Radius::Full)] radius: Radius,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button label icon_only=true disabled on_click={on_click}>|handle|
            <SwatchButtonFace handle color selected radius />
        </unstyled::Button>
    }
}

#[component]
fn SwatchButtonFace(
    handle: unstyled::ButtonHandle,
    color: Prop<Color32>,
    selected: Prop<bool>,
    radius: Radius,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        focused,
        tooltip,
        ..
    } = handle;
    let keyboard = focus_ring(focused);
    let ring = create_memo(move || {
        let color = match selected.get() || keyboard.get() {
            true => ColorRole::Accent,
            false => ColorRole::Border,
        };
        (selected.get() || keyboard.get() || hovered.get())
            .then(|| Border::new(color).width(Stroke::Focus))
    });
    let fill = color.map(ColorRole::Custom);
    view! {
        <Tooltip label={tooltip}>
            <Frame padding=Space::Xs radius border={ring}>
                <Frame width=Size::Sm height=Size::Sm radius fill={fill} border={Border::DEFAULT} />
            </Frame>
        </Tooltip>
    }
}
