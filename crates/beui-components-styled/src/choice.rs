use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::font::Overflow;
use beui_core::node::NodeId;
use beui_macros::{component, view};

use crate::select::Select;
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled::{ChoiceOption, ChoiceOptionHandle};
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::text::{Text, TextColor};
pub(super) use beui_components_unstyled::ChoiceKind as Kind;

pub(crate) const OPTION_SPACING: f32 = 6.0;
const MARK_SPACING: f32 = 10.0;
const MARK_BOX: f32 = 18.0;
const MARK_DOT: f32 = 8.0;
const MARK_RADIUS: f32 = 9.0;
const SEGMENT_PADDING_HORIZONTAL: f32 = 12.0;
const SEGMENT_PADDING_VERTICAL: f32 = 4.0;

#[component]
pub(super) fn OptionFace(kind: Kind, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || background(&theme, selected.get(), hovered.get()));
    view! {
        <Frame
            fill={ColorRole::custom(fill_color)}
            border={focus_border(focused, Border::FOCUS.offset(1.0))}
            radius=Radius::Md
            padding={Sides::symmetric(14.0, 6.0)}
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
    let theme = use_theme();
    let ring_color = create_memo(move || ring(&theme, checked.get()));
    view! {
        <Frame
            width=MARK_BOX
            height=MARK_BOX
            border={create_memo(move || Some(Border::new(ColorRole::Custom(ring_color.get())).width(2.0)))}
            radius={Radius::custom(MARK_RADIUS)}
            align=Align::Center
        >
            <Layers>
                <Show condition={checked}>
                    <Frame
                        width=MARK_DOT
                        height=MARK_DOT
                        fill=ColorRole::Accent
                        radius={Radius::custom(MARK_RADIUS)}
                    />
                </Show>
            </Layers>
        </Frame>
    }
}

fn ring(theme: &ThemeStore, checked: bool) -> Color32 {
    if checked {
        theme.accent.get()
    } else {
        theme.border.get()
    }
}

fn background(theme: &ThemeStore, active: bool, hovered: bool) -> Color32 {
    match (active, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.hover.get(),
        _ => Color32::TRANSPARENT,
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
    let theme = use_theme();
    let label_color = create_memo(move || match selected.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill_color = create_memo(move || background(&theme, selected.get(), hovered.get()));
    view! {
        <Frame
            fill={ColorRole::custom(fill_color)}
            border={focus_border(focused, Border::FOCUS.offset(1.0))}
            radius=Radius::Sm
            padding={Sides::symmetric(SEGMENT_PADDING_HORIZONTAL, SEGMENT_PADDING_VERTICAL)}
        >
            <Text overflow=Overflow::Ellipsis color={label_color} align=TextAlign::Center>
                {label}
            </Text>
        </Frame>
    }
}
