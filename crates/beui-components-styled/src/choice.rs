use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::types::Overflow;
use beui_view::types::TextAlign;

use crate::list::List;
use crate::select::Select;
use crate::tokens::{Padding, Size, Space, Stroke};
use beui_components_unstyled::{ChoiceOption, ChoiceOptionHandle};
use beui_view::primitives::{FirstFit, Layers};
use beui_view::reactive::*;

use crate::text::{Text, TextColor};
pub(super) use beui_components_unstyled::ChoiceKind as Kind;

pub(crate) const OPTION_SPACING: Space = Space::Sm;
const MARK_SPACING: Space = Space::Md;
const MARK_BOX: Size = Size::Custom(18.0);
const MARK_DOT: Size = Size::Custom(8.0);
const OPTION_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Sm);
const SEGMENT_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Xs);

#[component]
pub(super) fn OptionFace(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || background(selected.get(), hovered.get()));
    view! {
        <Frame
            fill={fill_color}
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Hairline))}
            radius=Radius::Md
            padding=OPTION_PADDING
        >
            <ChoiceLabel kind label color={label_color} checked={selected} />
        </Frame>
    }
}

#[component]
fn ChoiceLabel(
    kind: Kind,
    label: Prop<String>,
    color: Prop<TextColor>,
    checked: Memo<bool>,
) -> NodeId {
    let align = if kind == Kind::Tabs {
        TextAlign::Center
    } else {
        TextAlign::Start
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=MARK_SPACING>
            <Show condition={kind == Kind::Radio}>
                <RadioMark checked={checked} />
            </Show>
            <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0) color align>
                {label}
            </Text>
        </List>
    }
}

#[component]
fn RadioMark(checked: Memo<bool>) -> NodeId {
    let ring_color =
        create_memo(move || Some(Border::new(ring(checked.get())).width(Stroke::Focus)));
    view! {
        <Frame
            width=MARK_BOX
            height=MARK_BOX
            border={ring_color}
            radius=Radius::Full
            align=Align::Center
        >
            <Layers>
                <Show condition={checked}>
                    <Frame
                        width=MARK_DOT
                        height=MARK_DOT
                        fill=ColorRole::Accent
                        radius=Radius::Full
                    />
                </Show>
            </Layers>
        </Frame>
    }
}

fn ring(checked: bool) -> ColorRole {
    if checked {
        ColorRole::Accent
    } else {
        ColorRole::Border
    }
}

fn background(active: bool, hovered: bool) -> ColorRole {
    match (active, hovered) {
        (true, _) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Hover,
        _ => ColorRole::None,
    }
}

pub(super) struct Wide {
    pub options: Run<ChoiceOption>,
    pub selected: Binding<Option<usize>>,
}

#[component]
pub(super) fn Collapsing(
    options: Children<ChoiceOption>,
    selected: Binding<usize>,
    label: Memo<String>,
    #[prop(children)] wide: Render<Wide>,
) -> NodeId {
    let options = options.into_run();
    let count = options.clone();
    let shown = Prop::dynamic(move || Some(selected.get().min(count.len().saturating_sub(1))));
    let chosen = Binding::new(shown, move |chosen: Option<usize>| {
        if let Some(chosen) = chosen {
            selected.set(chosen);
        }
    });
    let narrow = options.clone();
    let wide = wide.call(Wide {
        options,
        selected: chosen,
    });
    view! {
        <FirstFit>
            {wide}
            <Select options={narrow} selected={chosen} label />
        </FirstFit>
    }
}

#[component]
pub(super) fn SegmentFace(handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || background(selected.get(), hovered.get()));
    view! {
        <Frame
            fill={fill_color}
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Hairline))}
            radius=Radius::Sm
            padding=SEGMENT_PADDING
        >
            <Text overflow=Overflow::Ellipsis color={label_color} align=TextAlign::Center>
                {label}
            </Text>
        </Frame>
    }
}
