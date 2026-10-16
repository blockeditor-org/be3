use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::border::Separator;
use crate::button::ButtonVariant;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::scroll::Scroll;
use crate::text::{Text, TextStyle};
use beui_components_unstyled as unstyled;
use beui_core::geometry::Vec2;
use beui_core::icons::ICON_RESET_SETTINGS;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PAGE_MAX_WIDTH: f32 = 680.0;
const PAGE_PADDING_HORIZONTAL: f32 = 20.0;
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
const RESET_WIDTH: f32 = 28.0;
const SECTION_TITLE: TextStyle = TextStyle::BODY.bold();

#[component]
pub fn SettingsPage(children: Children<ListChild>) -> NodeId {
    view! {
        <Frame fill=ColorRole::Background>
            <Scroll>
                <Frame
                    padding={Sides::symmetric(PAGE_PADDING_HORIZONTAL, PAGE_PADDING_VERTICAL)}
                    align={Align2::new(Align::Center, Align::Start)}
                >
                    <Frame width={Extent::at_most(PAGE_MAX_WIDTH)}>
                        <List spacing=PAGE_SPACING>{children}</List>
                    </Frame>
                </Frame>
            </Scroll>
        </Frame>
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
    view! {
        <List spacing=SECTION_SPACING>
            <Show condition={headed}>
                <Frame padding={Sides::horizontal(HEADER_INSET)}>
                    <List spacing=HEADER_SPACING>
                        <Show condition={titled}>
                            <Text style=SECTION_TITLE string={title} wrap=true />
                        </Show>
                        <Show condition={described}>
                            <Text style=TextStyle::CAPTION string={description} wrap=true />
                        </Show>
                    </List>
                </Frame>
            </Show>
            <Frame fill=ColorRole::Surface border={Border::DEFAULT} radius=Radius::Lg>
                <List spacing=0.0>{children}</List>
            </Frame>
        </List>
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
    #[prop(default = String::new())] reset_test_id: Prop<String>,
    on_reset: ClickCallback,
    children: Children<ListChild>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let description = create_memo(move || description.get());
    let described = create_memo(move || !description.get().is_empty());
    let separated = create_memo(move || separated.get());
    let reset_test_id = create_memo(move || reset_test_id.get());
    let modified = create_memo(move || modified.get());
    let resets = !on_reset.is_empty();
    let reset_off = create_memo(move || !modified.get() || locked.get());
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
                    <unstyled::Container>
                        {move |size: ReadSignal<Vec2>| {
                            let narrow = create_memo(move || {
                                let width = size.get().x;
                                !compact && width > 0.0 && width < STACK_BELOW
                            });
                            let spacing = create_memo(move || match narrow.get() {
                                true => STACKED_SPACING,
                                false => ROW_SPACING,
                            });
                            view! {
                                <unstyled::Stack spacing={spacing} narrow={narrow}>
                                    <List
                                        @sizing={label_size}
                                        justify=Justify::Center
                                        spacing=LABEL_SPACING
                                    >
                                        <Text string={label} wrap=true />
                                        <Show condition={described}>
                                            <Text
                                                style=TextStyle::CAPTION
                                                string={description}
                                                wrap=true
                                            />
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
                                        <Frame width=RESET_WIDTH align=Align2::CENTER>
                                            <List spacing=0.0>
                                                <Show condition={resets}>
                                                    <IconButton
                                                        glyph={ICON_RESET_SETTINGS.to_owned()}
                                                        label="Use the default"
                                                        variant=ButtonVariant::Ghost
                                                        size=IconButtonSize::Compact
                                                        disabled={reset_off}
                                                        @test_id={reset_test_id}
                                                        on_click={on_reset}
                                                    />
                                                </Show>
                                            </List>
                                        </Frame>
                                    </List>
                                </unstyled::Stack>
                            }
                        }}
                    </unstyled::Container>
                </Frame>
            </List>
        </List>
    }
}
