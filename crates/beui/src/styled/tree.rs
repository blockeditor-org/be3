use std::hash::Hash;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{
    Callback, CenteredRow, Frame, Func, ItemSize, Prop, RenderFn, Spacer, Text, clone, create_memo,
    intrinsic, percent, size,
};
use crate::styled::theme::{FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::{TreeItem, TreeRowHandle};

const INDENT: f32 = 12.0;
const MARKER_WIDTH: f32 = 14.0;
const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 6.0;
const PADDING_VERTICAL: f32 = 3.0;
const OUTLINE_WIDTH: f32 = 2.0;

#[component]
pub fn Tree<K>(
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] reveal: Prop<Option<K>>,
    #[prop(default = 0.0)] spacing: f32,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_hover_change: Callback<(K, bool)>,
    #[prop(children)] content: Option<RenderFn<K>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let content = content.expect("tree requires a `content` builder");
    view! {
        <unstyled::Tree
            keys
            item
            selected
            reveal
            spacing
            on_select={move |key| on_select.call(key)}
            on_expand={move |expansion| on_expand.call(expansion)}
            on_hover_change={move |hover| on_hover_change.call(hover)}
        >
            {move |handle: TreeRowHandle<K>| view! {
                <TreeRowFace handle content={content.clone()} />
            }}
        </unstyled::Tree>
    }
}

#[component]
fn TreeRowFace<K>(handle: TreeRowHandle<K>, content: RenderFn<K>) -> NodeId
where
    K: Clone + 'static,
{
    let TreeRowHandle {
        key,
        item,
        selected,
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected -> move || {
        background(&theme, selected.get(), hovered.get(), active.get())
    }));
    let indent = create_memo(clone!(item -> move || {
        ItemSize::Fixed(item.get().depth as f32 * INDENT)
    }));
    let glyph = create_memo(clone!(item -> move || {
        let item = item.get();
        marker(item.expandable, item.expanded).to_owned()
    }));
    let marker_color = theme.text_muted.clone();
    let spacer = view! {
        <Spacer />
    };
    let marker = view! {
        <Frame width=MARKER_WIDTH>
            <Text
                string=glyph
                font_size=FONT_SMALL
                color=marker_color
                monospace=true
                align=TextAlign::Center
            />
        </Frame>
    };
    let cells = vec![
        size(spacer, indent),
        intrinsic(marker),
        percent(content.call(key), 100.0),
    ];
    view! {
        <Frame
            color=fill
            outline={theme.accent.clone()}
            outline_width=OUTLINE_WIDTH
            radius=RADIUS
            outline_visible=focused
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <CenteredRow spacing=SPACING children=cells />
        </Frame>
    }
}

fn marker(expandable: bool, expanded: bool) -> &'static str {
    match (expandable, expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}

fn background(theme: &ThemeStore, selected: bool, hovered: bool, active: bool) -> Color32 {
    match (selected, hovered, active) {
        (_, _, true) => theme.pressed.get(),
        (true, _, _) => theme.accent_soft.get(),
        (false, true, _) => theme.hover.get(),
        (false, false, false) => Color32::TRANSPARENT,
    }
}
