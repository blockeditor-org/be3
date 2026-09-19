use beui::NodeId;
use beui::reactive::{
    Align, Children, Direction, Frame, ItemSize, List, ListChild, Prop, Scroll, clone, component,
    create_memo, view,
};
use beui::styled::theme::BORDER_WIDTH;
use beui::styled::{Separator, use_theme};

pub const SIDEBAR_WIDTH: f32 = 260.0;

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;
const BAND_PADDING_HORIZONTAL: f32 = 12.0;
const BAND_PADDING_VERTICAL: f32 = 8.0;
const BAND_SPACING: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[component]
pub fn Sidebar(
    #[prop(default = Side::Right)] side: Side,
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = SIDEBAR_WIDTH)] width: Prop<f32>,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let theme = use_theme();
    let shown = create_memo(move || shown.get());
    let leading = create_memo(clone!(shown -> move || shown.get() && side == Side::Right));
    let trailing = create_memo(clone!(shown -> move || shown.get() && side == Side::Left));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame visible={leading} width=BORDER_WIDTH color={theme.border.clone()} />
            <Frame visible={shown} width={width} color={theme.surface.clone()}>
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) focus_color={theme.accent.clone()}>
                        <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                            <List spacing=SPACING children={children} />
                        </Frame>
                    </Scroll>
                </List>
            </Frame>
            <Frame visible={trailing} width=BORDER_WIDTH color={theme.border.clone()} />
        </List>
    }
}

#[component]
pub fn Toolbar(
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = BAND_SPACING)] spacing: Prop<f32>,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let shown = create_memo(move || shown.get());
    let theme = use_theme();
    view! {
        <Frame visible={shown} color={theme.surface.clone()}>
            <List spacing=0.0>
                <Frame
                    padding_horizontal=BAND_PADDING_HORIZONTAL
                    padding_vertical=BAND_PADDING_VERTICAL
                >
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing={spacing}
                        children={children}
                    />
                </Frame>
                <Separator />
            </List>
        </Frame>
    }
}
