use beui::Key;
use beui::NodeId;
use beui::icons::{
    ICON_ADD_BOX, ICON_CHECKLIST, ICON_CODE, ICON_DATA_ARRAY, ICON_FORMAT_BOLD, ICON_FORMAT_ITALIC,
    ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED, ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE,
    ICON_KEYBOARD_HIDE, ICON_LINK, ICON_SETTINGS, ICON_TITLE,
};
use beui::reactive::{
    Action, Align, Chord, Direction, ForEach, Frame, ItemSize, List, Show, WriteSignal, clone,
    component, create_memo, create_signal, view, with_document,
};
use beui::styled::{Body, Dialog, IconButton, NumberInput, Select, Separator, Switch, use_theme};
use beui::unstyled::{ChoiceOption, Scroll};
use block_editor_beui::BlockParent;
use block_editor_beui::{BlockFilter, block_ui::BlockLabel};
use text_editor_core::{EditorCommand, MarkdownCommand, TextIndentation, TextLanguage};

use super::state::Shared;

const TOOLBAR_SPACING: f32 = 6.0;
const SETTINGS_SPACING: f32 = 10.0;
const SELECT_WIDTH: f32 = 120.0;
const NUMBER_WIDTH: f32 = 70.0;
const FORMAT_PADDING: f32 = 6.0;

#[component]
pub(crate) fn EditorMenu(state: Shared) -> NodeId {
    let (settings, set_settings) = create_signal(false);
    menu_actions(&state, set_settings.clone());
    let hex = state.hex_view.clone();
    let text_view = create_memo(clone!(hex -> move || !hex.get()));
    let hex_view = create_memo(move || hex.get());
    let text_state = state.clone();
    view! {
        <Dialog open={settings} title="Text settings" on_dismiss={move || set_settings.set(false)}>
            <List spacing=SETTINGS_SPACING>
                <Show condition={text_view}>
                    {move || view! {
                        <TextSettings state={text_state.clone()} />
                    }}
                </Show>
                <Show condition={hex_view}>
                    {move || view! {
                        <HexSettings state={state.clone()} />
                    }}
                </Show>
            </List>
        </Dialog>
    }
}

fn menu_actions(state: &Shared, set_settings: WriteSignal<bool>) {
    let hex = state.hex_view.clone();
    let in_text = create_memo(clone!(hex -> move || !hex.get()));
    Action::new(
        "text.hex",
        "Toggle hex view",
        clone!(state -> move || state.toggle_hex_view()),
    )
    .glyph(ICON_DATA_ARRAY)
    .checked(create_memo(move || hex.get()))
    .in_menu()
    .register();
    Action::new(
        "text.insert-block",
        "Insert a block",
        clone!(state -> move || pick_block(&state)),
    )
    .glyph(ICON_ADD_BOX)
    .enabled(in_text)
    .in_menu()
    .register();
    Action::new("text.settings", "Text settings", move || {
        set_settings.set(true)
    })
    .glyph(ICON_SETTINGS)
    .in_menu()
    .register();
}

const FORMATS: [(&str, &str, &str, MarkdownCommand, Option<Chord>); 10] = [
    (
        "text.format.heading",
        "Heading",
        ICON_TITLE,
        MarkdownCommand::Heading(2),
        None,
    ),
    (
        "text.format.bold",
        "Bold",
        ICON_FORMAT_BOLD,
        MarkdownCommand::Bold,
        Some(Chord::ctrl(Key::B)),
    ),
    (
        "text.format.italic",
        "Italic",
        ICON_FORMAT_ITALIC,
        MarkdownCommand::Italic,
        Some(Chord::ctrl(Key::I)),
    ),
    (
        "text.format.bulleted-list",
        "Bulleted list",
        ICON_FORMAT_LIST_BULLETED,
        MarkdownCommand::BulletedList,
        None,
    ),
    (
        "text.format.checklist",
        "Checklist",
        ICON_CHECKLIST,
        MarkdownCommand::Checklist,
        None,
    ),
    (
        "text.format.link",
        "Link",
        ICON_LINK,
        MarkdownCommand::Link,
        None,
    ),
    (
        "text.format.strikethrough",
        "Strikethrough",
        ICON_FORMAT_STRIKETHROUGH,
        MarkdownCommand::Strikethrough,
        None,
    ),
    (
        "text.format.inline-code",
        "Inline code",
        ICON_CODE,
        MarkdownCommand::InlineCode,
        None,
    ),
    (
        "text.format.numbered-list",
        "Numbered list",
        ICON_FORMAT_LIST_NUMBERED,
        MarkdownCommand::NumberedList,
        None,
    ),
    (
        "text.format.image",
        "Image",
        ICON_IMAGE,
        MarkdownCommand::Image,
        None,
    ),
];

pub(crate) fn format_actions(state: &Shared) -> Vec<Action> {
    let content = state.text.content();
    let hex = state.hex_view.clone();
    let markdown = create_memo(clone!(state -> move || {
        content.get();
        !hex.get() && state.text.language() == TextLanguage::Markdown
    }));
    FORMATS
        .iter()
        .map(|(id, label, glyph, command, chord)| {
            let command = *command;
            let mut action = Action::new(
                *id,
                *label,
                clone!(state -> move || state.text.execute(EditorCommand::Markdown(command))),
            )
            .glyph(glyph)
            .enabled(markdown.clone());
            if let Some(chord) = chord {
                action = action.shortcut(*chord);
            }
            action.register()
        })
        .collect()
}

#[component]
pub(crate) fn FormatBar(state: Shared, formats: Vec<Action>) -> NodeId {
    let theme = use_theme();
    let typing = state.typing.clone();
    let hex = state.hex_view.clone();
    let shown = create_memo(move || typing.get() && !hex.get());
    let touch = state.text.touch_mode();
    let keyboard = create_memo(move || touch.get());
    let content = state.text.content();
    let markdown = create_memo(clone!(state -> move || {
        content.get();
        state.text.language() == TextLanguage::Markdown
    }));
    let controls = move || {
        let formats = formats.clone();
        view! {
            <MarkdownControls formats />
        }
    };
    let bar = move || {
        let controls = controls.clone();
        let keyboard = keyboard.clone();
        let markdown = markdown.clone();
        view! {
            <Frame color={theme.surface.clone()}>
                <List spacing=0.0>
                    <Separator />
                    <Frame padding_vertical=FORMAT_PADDING>
                        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                            <Scroll
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                            >
                                <Frame padding_horizontal=FORMAT_PADDING>
                                    <List spacing=0.0>
                                        <Show condition={markdown.clone()} then={controls} />
                                    </List>
                                </Frame>
                            </Scroll>
                            <Show condition={keyboard}>
                                <Frame padding_horizontal=FORMAT_PADDING>
                                    <IconButton
                                        glyph=ICON_KEYBOARD_HIDE
                                        label="Done"
                                        press_focus=false
                                        @test_id={"text.format.done"}
                                        on_click={|| with_document(|document| document.update_focus(None))}
                                    />
                                </Frame>
                            </Show>
                        </List>
                    </Frame>
                </List>
            </Frame>
        }
    };
    view! {
        <List spacing=0.0>
            <Show condition={shown} then={bar} />
        </List>
    }
}

#[component]
fn MarkdownControls(formats: Vec<Action>) -> NodeId {
    let count = formats.len();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <ForEach keys={(0..count).collect::<Vec<usize>>()}>
                {move |index: usize| {
                    let action = formats[index].clone();
                    let id = action.id().to_owned();
                    view! {
                        <IconButton @test_id={id} action press_focus=false />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn HexSettings(state: Shared) -> NodeId {
    let insert = state.hex_insert_mode.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
            <Body @sizing=ItemSize::Percent(100.0) content="Insert instead of overwriting" />
            <Switch
                label="Insert instead of overwriting"
                on={insert}
                @test_id={"text.hex-insert"}
                on_change={move |pressed: bool| {
                    state.set_hex_insert_mode.set(pressed);
                    state.hex_pending_nibble.set(None);
                }}
            />
        </List>
    }
}

#[component]
fn TextSettings(state: Shared) -> NodeId {
    let content = state.text.content();
    let language = create_memo(clone!(state content -> move || {
        content.get();
        state.text.language()
    }));
    let indentation = create_memo(clone!(state content -> move || {
        content.get();
        state.text.indentation()
    }));
    let language_index = create_memo(clone!(language -> move || {
        TextLanguage::ALL
            .iter()
            .position(|choice| *choice == language.get())
    }));
    let indentation_index = create_memo(clone!(indentation -> move || {
        Some(usize::from(matches!(
            indentation.get(),
            TextIndentation::Spaces { .. }
        )))
    }));
    let spaces = create_memo(clone!(indentation -> move || {
        matches!(indentation.get(), TextIndentation::Spaces { .. })
    }));
    let width = create_memo(clone!(indentation -> move || match indentation.get() {
        TextIndentation::Spaces { width } => f64::from(width),
        TextIndentation::Tabs => 2.0,
    }));
    let language_state = state.clone();
    let indentation_state = state.clone();
    let width_state = state;
    view! {
        <List spacing=SETTINGS_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <Body @sizing=ItemSize::Percent(100.0) content="Language" />
                <Frame width=SELECT_WIDTH>
                    <Select
                        label="Language"
                        selected={language_index}
                        @test_id={"text.language"}
                        options={view! {
                            <ForEach keys={(0..TextLanguage::ALL.len()).collect::<Vec<usize>>()}>
                                {move |index: usize| view! {
                                    <ChoiceOption
                                        label={TextLanguage::ALL[index].label().to_owned()}
                                    />
                                }}
                            </ForEach>
                        }}
                        on_change={move |index: Option<usize>| {
                            let Some(choice) = index.and_then(|index| TextLanguage::ALL.get(index)) else {
                                return;
                            };
                            language_state.text.execute(EditorCommand::SetLanguage(*choice));
                        }}
                    />
                </Frame>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <Body @sizing=ItemSize::Percent(100.0) content="Indentation" />
                <Frame width=SELECT_WIDTH>
                    <Select
                        label="Indentation"
                        selected={indentation_index}
                        @test_id={"text.indentation"}
                        options={view! {
                            <ChoiceOption label="Tabs" />
                            <ChoiceOption label="Spaces" />
                        }}
                        on_change={move |index: Option<usize>| {
                            let indentation = match index {
                                Some(1) => TextIndentation::Spaces { width: 2 },
                                _ => TextIndentation::Tabs,
                            };
                            indentation_state.text.execute(EditorCommand::SetIndentation(indentation));
                        }}
                    />
                </Frame>
            </List>
            <Show condition={spaces}>
                {move || clone!(width width_state -> view! {
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=SETTINGS_SPACING
                    >
                        <Body @sizing=ItemSize::Percent(100.0) content="Indentation width" />
                        <Frame width=NUMBER_WIDTH>
                            <NumberInput
                                label="Indentation width"
                                value={width.clone()}
                                min=1.0
                                max=8.0
                                @test_id={"text.indentation-width"}
                                on_change={clone!(width_state -> move |value: f64| {
                                    let width = value.round().clamp(1.0, 8.0) as u8;
                                    width_state.text.execute(EditorCommand::SetIndentation(
                                        TextIndentation::Spaces { width },
                                    ));
                                })}
                            />
                        </Frame>
                    </List>
                })}
            </Show>
        </List>
    }
}

fn pick_block(state: &Shared) {
    let picked = state.clone();
    state.editor.pick_block(
        BlockFilter {
            name: "Insert".to_owned(),
            block_types: Vec::new(),
            excluded: Vec::new(),
            templates: false,
            place: None,
        },
        move |result| {
            let Ok(block) = result else {
                return;
            };
            picked
                .client
                .set_parent(block.id, BlockParent::Block(picked.block_id));
            let types = picked.host().block_types();
            let name = match picked.client.info(block.id) {
                Some(info) => info.label(types.as_ref()).name,
                None => BlockLabel::new(types.as_ref(), block.block_type, None, false).name,
            };
            picked.insert_image_embed(block.id, &name);
        },
    );
}
