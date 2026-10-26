use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use crate::text::{Icon, TextColor};
use crate::theme::use_theme;
use beui_core::color::Color32;
use beui_core::icons::{ICON_CHEVRON_RIGHT, ICON_ELLIPSIS};
use beui_macros::{component, view};

use crate::border::Separator;
use crate::icon_button::IconButtonSize;
use crate::menu_button::IconMenuButton;
use crate::scroll::Scroll;
use crate::text::{Text, TextStyle};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::MenuItem;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PAGE_MAX_WIDTH: f32 = 680.0;
const SECTION_INSET: f32 = 20.0;
const PAGE_PADDING_VERTICAL: f32 = 24.0;
const PAGE_SPACING: f32 = 28.0;
const SECTION_SPACING: f32 = 10.0;
const HEADER_SPACING: f32 = 2.0;
const HEADER_INSET: f32 = 4.0;
const ROW_PADDING_HORIZONTAL: f32 = 16.0;
const ROW_PADDING_VERTICAL: f32 = 12.0;
const ROW_SPACING: f32 = 16.0;
const STACKED_SPACING: f32 = 10.0;
const LABEL_SPACING: f32 = 2.0;
const CONTROL_SPACING: f32 = 8.0;
const STACK_BELOW: f32 = 400.0;
const LABEL_WEIGHT: f32 = 55.0;
const CONTROL_WEIGHT: f32 = 45.0;
const MARKER_WIDTH: f32 = 3.0;
const EDGE_TO_EDGE_BELOW: f32 = 520.0;
const LINK_INSET: f32 = 4.0;
const SECTION_TITLE: TextStyle = TextStyle::BODY.bold();
const NOTE: TextStyle = TextStyle::CAPTION.color(TextColor::Accent);
const MENU_INSET: f32 = 8.0;

#[component]
pub fn SettingsPage(#[prop(children)] sections: Render<(), Children<ListChild>>) -> NodeId {
    view! {
        <Scroll>
            <Frame
                padding={Sides::vertical(PAGE_PADDING_VERTICAL)}
                align={Align2::new(Align::Center, Align::Start)}
            >
                <Frame width={Extent::at_most(PAGE_MAX_WIDTH + 2.0 * SECTION_INSET)}>
                    <unstyled::Container>|_size|
                        <List spacing=PAGE_SPACING children={sections.call(())} />
                    </unstyled::Container>
                </Frame>
            </Frame>
        </Scroll>
    }
}

#[component]
pub fn SettingsSection(
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = String::new())] description: Prop<String>,
    children: Children<ListChild>,
) -> NodeId {
    let title = create_memo(move || title.get());
    let description = create_memo(move || description.get());
    let titled = create_memo(move || !title.get().is_empty());
    let described = create_memo(move || !description.get().is_empty());
    let headed = create_memo(move || titled.get() || described.get());
    let edge = unstyled::narrower_than(EDGE_TO_EDGE_BELOW);
    let inset = create_memo(move || match edge.get() {
        true => Sides::ZERO,
        false => Sides::horizontal(SECTION_INSET),
    });
    let header_inset = create_memo(move || match edge.get() {
        true => Sides::horizontal(ROW_PADDING_HORIZONTAL),
        false => Sides::horizontal(HEADER_INSET),
    });
    let border = create_memo(move || match edge.get() {
        true => None,
        false => Some(Border::DEFAULT),
    });
    let radius = create_memo(move || match edge.get() {
        true => Radius::None,
        false => Radius::Lg,
    });
    view! {
        <Frame padding={inset}>
            <List spacing=SECTION_SPACING>
                <Show condition={headed}>
                    <Frame padding={header_inset}>
                        <List spacing=HEADER_SPACING>
                            <Show condition={titled}>
                                <Text style=SECTION_TITLE>{title}</Text>
                            </Show>
                            <Show condition={described}>
                                <Text style=TextStyle::CAPTION>{description}</Text>
                            </Show>
                        </List>
                    </Frame>
                </Show>
                <List spacing=0.0>
                    <Show condition={edge}>
                        <Separator />
                    </Show>
                    <Frame fill=ColorRole::Surface border={border} radius={radius}>
                        <List spacing=0.0>{children}</List>
                    </Frame>
                    <Show condition={edge}>
                        <Separator />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

#[component]
pub fn SettingsRow(
    label: Prop<String>,
    #[prop(default = String::new())] description: Prop<String>,
    #[prop(default = false)] separated: Prop<bool>,
    #[prop(default = false)] compact: bool,
    #[prop(default = false)] modified: Prop<bool>,
    #[prop(default = false)] locked: Prop<bool>,
    #[prop(default = String::new())] note: Prop<String>,
    #[prop(default = String::new())] menu_test_id: Prop<String>,
    menu: Children<MenuItem>,
    children: Children<ListChild>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let description = create_memo(move || description.get());
    let described = create_memo(move || !description.get().is_empty());
    let note = create_memo(move || note.get());
    let noted = create_memo(move || !note.get().is_empty());
    let separated = create_memo(move || separated.get());
    let menu_test_id = create_memo(move || menu_test_id.get());
    let modified = create_memo(move || modified.get());
    let has_menu = !menu.is_empty();
    let locked = create_memo(move || locked.get());
    let marker = create_memo(move || match modified.get() {
        true => ColorRole::Accent,
        false => ColorRole::None,
    });
    let (label_size, control_size) = match compact {
        true => (ItemSize::Percent(100.0), ItemSize::Intrinsic),
        false => (
            ItemSize::Percent(LABEL_WEIGHT),
            ItemSize::Percent(CONTROL_WEIGHT),
        ),
    };
    let menu_button = has_menu.then(|| {
        view! {
            <Frame
                padding={Sides::new(0.0, ROW_PADDING_VERTICAL, MENU_INSET, ROW_PADDING_VERTICAL)}
                align={Align2::new(Align::Center, Align::Center)}
            >
                <IconMenuButton
                    label="More"
                    glyph=ICON_ELLIPSIS
                    size=IconButtonSize::Compact
                    disabled={locked}
                    @test_id={menu_test_id}
                    items={menu}
                />
            </Frame>
        }
    });
    let narrow = unstyled::narrower_than(STACK_BELOW + 2.0 * ROW_PADDING_HORIZONTAL);
    let narrow = create_memo(move || !compact && narrow.get());
    let spacing = create_memo(move || match narrow.get() {
        true => STACKED_SPACING,
        false => ROW_SPACING,
    });
    view! {
        <List spacing=0.0>
            <Show condition={separated}>
                <Frame padding={Sides::horizontal(ROW_PADDING_HORIZONTAL)}>
                    <Separator />
                </Frame>
            </Show>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame padding={Sides::vertical(ROW_PADDING_VERTICAL)}>
                    <Frame width=MARKER_WIDTH fill={marker} radius=Radius::Full />
                </Frame>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding={Sides::symmetric(ROW_PADDING_HORIZONTAL - MARKER_WIDTH, ROW_PADDING_VERTICAL)}
                >
                    <unstyled::Stack spacing={spacing} narrow={narrow}>
                        <List @sizing={label_size} justify=Justify::Center spacing=LABEL_SPACING>
                            <Text>{label}</Text>
                            <Show condition={described}>
                                <Text style=TextStyle::CAPTION>{description}</Text>
                            </Show>
                            <Show condition={noted}>
                                <Text style=NOTE>{note}</Text>
                            </Show>
                        </List>
                        <List
                            @sizing={control_size}
                            direction=Direction::Horizontal
                            align=Align::Center
                            justify=Justify::End
                            spacing=CONTROL_SPACING
                        >
                            {children}
                        </List>
                    </unstyled::Stack>
                </Frame>
                {menu_button}
            </List>
        </List>
    }
}

#[component]
pub fn SettingsLink(
    label: Prop<String>,
    #[prop(default = String::new())] description: Prop<String>,
    #[prop(default = false)] separated: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] modified: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let separated = create_memo(move || separated.get());
    let description = create_memo(move || description.get());
    let modified = create_memo(move || modified.get());
    let marker = create_memo(move || match modified.get() {
        true => ColorRole::Accent,
        false => ColorRole::None,
    });
    view! {
        <List spacing=0.0>
            <Show condition={separated}>
                <Frame padding={Sides::horizontal(ROW_PADDING_HORIZONTAL)}>
                    <Separator />
                </Frame>
            </Show>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame padding={Sides::vertical(ROW_PADDING_VERTICAL)}>
                    <Frame width=MARKER_WIDTH fill={marker} radius=Radius::Full />
                </Frame>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding={Sides::new(0.0, LINK_INSET, LINK_INSET, LINK_INSET)}
                >
                    <unstyled::Button label disabled on_click={on_click}>|handle|
                        <SettingsLinkFace handle description={description} />
                    </unstyled::Button>
                </Frame>
            </List>
        </List>
    }
}

#[component]
fn SettingsLinkFace(handle: unstyled::ButtonHandle, description: Memo<String>) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        label,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(
        move || match (disabled.get(), active.get(), hovered.get()) {
            (true, _, _) => Color32::TRANSPARENT,
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        },
    );
    let text_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let described = create_memo(move || !description.get().is_empty());
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            border={focus_border(focused, Border::FOCUS)}
            radius=Radius::Md
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding={Sides::new(
                        ROW_PADDING_HORIZONTAL - MARKER_WIDTH,
                        ROW_PADDING_VERTICAL - LINK_INSET,
                        ROW_PADDING_HORIZONTAL - LINK_INSET,
                        ROW_PADDING_VERTICAL - LINK_INSET,
                    )}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <List
                            @sizing=ItemSize::Percent(100.0)
                            justify=Justify::Center
                            spacing=LABEL_SPACING
                        >
                            <Text color={text_color}>{label}</Text>
                            <Show condition={described}>
                                <Text style=TextStyle::CAPTION>{description}</Text>
                            </Show>
                        </List>
                        <Text color=TextColor::Muted>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </List>
                </Frame>
            </List>
        </Frame>
    }
}
