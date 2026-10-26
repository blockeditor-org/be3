use crate::context_menu::MenuTreeLead;
use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use crate::text::{Icon, Text, TextColor, TextSize, TextStyle};
use crate::theme::use_theme;
use beui_components_unstyled::Picture;
use beui_core::color::Color32;
use beui_core::font::{Overflow, TextAlign};
use beui_core::icons::{Glyph, ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_DOT};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

const LABEL_SPACING: f32 = 2.0;
const TOUCH_HEIGHT: f32 = 52.0;
const LARGE_PICTURE: f32 = 32.0;
const TREE_INDENT: f32 = 20.0;
const TREE_CHEVRON: f32 = 28.0;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum RowSize {
    #[default]
    Compact,
    Regular,
    Large,
    Touch,
}

impl RowSize {
    fn padding(self) -> Sides {
        match self {
            RowSize::Compact => Sides::symmetric(8.0, 4.0),
            RowSize::Regular => Sides::new(13.0, 8.0, 12.0, 8.0),
            RowSize::Large => Sides::symmetric(10.0, 6.0),
            RowSize::Touch => Sides::horizontal(12.0),
        }
    }

    fn spacing(self) -> f32 {
        match self {
            RowSize::Compact => 8.0,
            RowSize::Regular => 16.0,
            RowSize::Large => 12.0,
            RowSize::Touch => 14.0,
        }
    }

    fn height(self) -> Extent {
        match self {
            RowSize::Touch => Extent::px(TOUCH_HEIGHT),
            _ => Extent::INTRINSIC,
        }
    }

    fn leading(self) -> f32 {
        match self {
            RowSize::Large => LARGE_PICTURE,
            _ => TextSize::Base.icon_px(),
        }
    }

    fn wraps(self) -> bool {
        self == RowSize::Regular
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum RowTrailing {
    #[default]
    None,
    Chevron,
    Check,
    Radio,
}

impl RowTrailing {
    fn glyph(self) -> Option<Glyph> {
        match self {
            RowTrailing::None => None,
            RowTrailing::Chevron => Some(ICON_CHEVRON_RIGHT),
            RowTrailing::Check => Some(ICON_CHECK),
            RowTrailing::Radio => Some(ICON_DOT),
        }
    }

    fn color(self) -> TextColor {
        match self {
            RowTrailing::Check | RowTrailing::Radio => TextColor::Accent,
            RowTrailing::None | RowTrailing::Chevron => TextColor::Muted,
        }
    }
}

#[component]
pub fn RowFace(
    #[prop(default = false)] hovered: Prop<bool>,
    #[prop(default = false)] active: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: Prop<bool>,
    #[prop(default = RowSize::Compact)] size: RowSize,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = String::new())] lead: Prop<String>,
    #[prop(default = String::new())] shortcut: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] glyph_color: Prop<Option<TextColor>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = false)] leading: Prop<bool>,
    #[prop(default = RowTrailing::None)] trailing: Prop<RowTrailing>,
    #[prop(default = None)] depth: Prop<Option<usize>>,
    #[prop(default = None)] expanded: Prop<Option<bool>>,
    toggle: ClickCallback,
    children: Option<Child>,
) -> NodeId {
    let theme = use_theme();
    let disabled = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let fill = create_memo(move || {
        if disabled.get() {
            return Color32::TRANSPARENT;
        }
        match (active.get(), hovered.get(), selected.get()) {
            (true, _, _) => theme.pressed.get(),
            (false, true, _) => theme.hover.get(),
            (false, false, true) => theme.accent_soft.get(),
            (false, false, false) => Color32::TRANSPARENT,
        }
    });
    let focused = create_memo(move || focused.get());
    let text_color = create_memo(move || match (disabled.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        (false, false) => TextColor::Default,
    });
    let icon_color = create_memo(move || match (disabled.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        _ => glyph_color.get().unwrap_or(TextColor::Muted),
    });
    let glyph = create_memo(move || glyph.get());
    let image = create_memo(move || image.get());
    let pictured = create_memo(move || image.with(Option::is_some));
    let plain = create_memo(move || !pictured.get());
    let has_leading = create_memo(move || leading.get() || pictured.get() || glyph.get().is_some());
    let lead = create_memo(move || lead.get());
    let has_lead = create_memo(move || !lead.get().is_empty());
    let shortcut = create_memo(move || shortcut.get());
    let has_shortcut = create_memo(move || !shortcut.get().is_empty());
    let trailing = create_memo(move || trailing.get());
    let trailing_glyph = create_memo(move || trailing.get().glyph());
    let trailing_color = create_memo(move || trailing.get().color());
    let has_trailing = create_memo(move || trailing.get() != RowTrailing::None);
    let side = size.leading();
    let fill = ColorRole::custom(fill);
    let border = focus_border(focused, Border::FOCUS);
    if let Some(children) = children {
        return view! {
            <Frame
                height={size.height()}
                fill={fill.clone()}
                radius=Radius::Md
                border={border}
                padding={size.padding()}
            >
                {children}
            </Frame>
        };
    }
    view! {
        <Frame
            height={size.height()}
            fill
            radius=Radius::Md
            border={border}
            padding={size.padding()}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing={size.spacing()}>
                <MenuTreeLead depth expanded toggle indent=TREE_INDENT chevron=TREE_CHEVRON />
                <Show condition={has_lead}>
                    <Text size=TextSize::Sm color=TextColor::Muted>{lead}</Text>
                </Show>
                <Show condition={has_leading}>
                    <Frame width=side>
                        <List spacing=0.0 align=Align::Center>
                            <Show condition={pictured}>
                                <Frame width=side height=side>
                                    <Picture image />
                                </Frame>
                            </Show>
                            <Show condition={plain}>
                                <Text color={icon_color} align=TextAlign::Center>
                                    <Icon glyph={glyph} />
                                </Text>
                            </Show>
                        </List>
                    </Frame>
                </Show>
                <Frame @sizing=ItemSize::Percent(100.0)>
                    <RowLabel size label detail color={text_color} />
                </Frame>
                <Show condition={has_shortcut}>
                    <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::End>
                        {shortcut}
                    </Text>
                </Show>
                <Show condition={has_trailing}>
                    <Text color={trailing_color} align=TextAlign::Center>
                        <Icon glyph={trailing_glyph} />
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn RowLabel(
    size: RowSize,
    label: Prop<String>,
    detail: Prop<String>,
    color: Memo<TextColor>,
) -> NodeId {
    let detail = create_memo(move || detail.get());
    let described = create_memo(move || !detail.get().is_empty());
    let overflow = match size.wraps() {
        true => Overflow::default(),
        false => Overflow::Ellipsis,
    };
    view! {
        <List justify=Justify::Center spacing=LABEL_SPACING>
            <Text color align=TextAlign::Start overflow>{label}</Text>
            <Show condition={described}>
                <Text style=TextStyle::CAPTION align=TextAlign::Start overflow>{detail}</Text>
            </Show>
        </List>
    }
}
