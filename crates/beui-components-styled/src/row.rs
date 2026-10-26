use crate::context_menu::MenuTreeLead;
use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use crate::text::{Icon, Text, TextColor, TextSize, TextStyle};
use beui_components_unstyled::Picture;
use beui_view::types::{Overflow, TextAlign};
use beui_view::types::icons::{Glyph, ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_DOT};
use beui_view::types::Image;
use beui_view::reactive::NodeId;
use beui_macros::{component, view};

use crate::list::List;
use crate::tokens::{Length, Padding, Size, Space};
use beui_view::reactive::*;

const LABEL_SPACING: Space = Space::Xs2;
const TREE_INDENT: Space = Space::Xl2;
const TREE_CHEVRON: Size = Size::Custom(28.0);

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum RowSize {
    #[default]
    Compact,
    Regular,
    Large,
    Touch,
}

impl RowSize {
    fn padding(self) -> Padding {
        match self {
            RowSize::Compact => Padding::symmetric(Space::Md, Space::Xs),
            RowSize::Regular => Padding::symmetric(Space::Lg, Space::Md),
            RowSize::Large => Padding::symmetric(Space::Md, Space::Sm),
            RowSize::Touch => Padding::horizontal(Space::Lg),
        }
    }

    fn spacing(self) -> Space {
        match self {
            RowSize::Compact => Space::Md,
            RowSize::Regular => Space::Xl,
            RowSize::Large | RowSize::Touch => Space::Lg,
        }
    }

    fn height(self) -> Length {
        match self {
            RowSize::Touch => Length::Size(Size::TouchRow),
            _ => Length::INTRINSIC,
        }
    }

    fn leading(self) -> Size {
        match self {
            RowSize::Large => Size::Lg,
            _ => Size::Custom(TextSize::Base.icon_px()),
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
    let disabled = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let fill = create_memo(move || {
        if disabled.get() {
            return ColorRole::None;
        }
        match (active.get(), selected.get(), hovered.get()) {
            (true, _, _) => ColorRole::Pressed,
            (false, true, true) => ColorRole::AccentSoftHover,
            (false, true, false) => ColorRole::AccentSoft,
            (false, false, true) => ColorRole::Hover,
            (false, false, false) => ColorRole::None,
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
    let fill: Prop<ColorRole> = fill.into_prop();
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
                        <List spacing=Space::None align=Align::Center>
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
