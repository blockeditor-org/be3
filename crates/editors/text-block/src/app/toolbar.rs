use beui::Key;
use beui::NodeId;
use beui::icons::{
    ICON_ADD_BOX, ICON_CHECKLIST, ICON_CODE, ICON_DATA_ARRAY, ICON_FORMAT_BOLD, ICON_FORMAT_ITALIC,
    ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED, ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE,
    ICON_KEYBOARD_HIDE, ICON_LINK, ICON_SETTINGS, ICON_TITLE,
};
use beui::reactive::{
    Action, Align, Chord, Direction, ForEach, Frame, Interactive, ItemSize, List, Show,
    WriteSignal, component, create_memo, create_signal, view, with_document,
};
use beui::styled::{Dialog, IconButton, NumberInput, Select, Separator, Switch, use_theme};
use beui::unstyled::{ChoiceOption, Scroll};
use block_editor_beui::BlockParent;
use block_editor_beui::{BlockFilter, block_ui::BlockLabel};
use text_editor_core::{EditorCommand, MarkdownCommand, TextIndentation, TextLanguage};

use super::state::State;
use beui::styled::Text;

const TOOLBAR_SPACING: f32 = 6.0;
const SETTINGS_SPACING: f32 = 10.0;
const SELECT_WIDTH: f32 = 120.0;
const NUMBER_WIDTH: f32 = 70.0;
const FORMAT_PADDING: f32 = 6.0;

#[component]
pub(crate) fn EditorMenu(state: State) -> NodeId {
    let (settings, set_settings) = create_signal(false);
    menu_actions(state, set_settings);
    let hex = state.hex_view;
    let text_view = create_memo(move || !hex.get());
    let hex_view = create_memo(move || hex.get());
    view! {
        <Dialog open={settings} title="Text settings" on_dismiss={move || set_settings.set(false)}>
            <List spacing=SETTINGS_SPACING>
                <Show condition={text_view}>
                    {move || view! {
                        <TextSettings state={state} />
                    }}
                </Show>
                <Show condition={hex_view}>
                    {move || view! {
                        <HexSettings state={state} />
                    }}
                </Show>
            </List>
        </Dialog>
    }
}

fn menu_actions(state: State, set_settings: WriteSignal<bool>) {
    let hex = state.hex_view;
    let in_text = create_memo(move || !hex.get());
    Action::new("text.hex", "Toggle hex view", move || {
        state.toggle_hex_view()
    })
    .glyph(ICON_DATA_ARRAY)
    .checked(create_memo(move || hex.get()))
    .in_menu()
    .register();
    Action::new("text.insert-block", "Insert a block", move || {
        pick_block(state)
    })
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

pub(crate) fn format_actions(state: State) -> Vec<Action> {
    let content = state.text.content();
    let hex = state.hex_view;
    let markdown = create_memo(move || {
        content.get();
        !hex.get() && state.text.language() == TextLanguage::Markdown
    });
    FORMATS
        .iter()
        .map(|(id, label, glyph, command, chord)| {
            let command = *command;
            let mut action = Action::new(*id, *label, move || {
                state.text.execute(EditorCommand::Markdown(command))
            })
            .glyph(glyph)
            .enabled(markdown);
            if let Some(chord) = chord {
                action = action.shortcut(*chord);
            }
            action.register()
        })
        .collect()
}

#[component]
pub(crate) fn FormatBar(state: State, formats: Vec<Action>) -> NodeId {
    let theme = use_theme();
    let typing = state.typing;
    let hex = state.hex_view;
    let shown = create_memo(move || typing.get() && !hex.get());
    let touch = state.text.touch_mode();
    let keyboard = create_memo(move || touch.get());
    let content = state.text.content();
    let markdown = create_memo(move || {
        content.get();
        state.text.language() == TextLanguage::Markdown
    });
    let controls = move || {
        let formats = formats.clone();
        view! {
            <MarkdownControls formats />
        }
    };
    let bar = move || {
        let controls = controls.clone();
        let keyboard = keyboard;
        let markdown = markdown;
        view! {
            <Interactive press_focus=false @test_id={"text.format"}>
                <Frame color={theme.surface}>
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
                                            <Show condition={markdown} then={controls} />
                                        </List>
                                    </Frame>
                                </Scroll>
                                <Show condition={keyboard}>
                                    <Frame padding_horizontal=FORMAT_PADDING>
                                        <IconButton
                                            glyph=ICON_KEYBOARD_HIDE
                                            label="Done"
                                            @test_id={"text.format.done"}
                                            on_click={|| with_document(|document| document.update_focus(None))}
                                        />
                                    </Frame>
                                </Show>
                            </List>
                        </Frame>
                    </List>
                </Frame>
            </Interactive>
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
                        <IconButton @test_id={id} action />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn HexSettings(state: State) -> NodeId {
    let insert = state.hex_insert_mode;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
            <Text @sizing=ItemSize::Percent(100.0) string="Insert instead of overwriting" />
            <Switch
                label="Insert instead of overwriting"
                on={insert}
                @test_id={"text.hex-insert"}
                on_change={move |pressed: bool| {
                    state.set_hex_insert_mode.set(pressed);
                    state.inner().hex_pending_nibble.set(None);
                }}
            />
        </List>
    }
}

#[component]
fn TextSettings(state: State) -> NodeId {
    let content = state.text.content();
    let language = create_memo(move || {
        content.get();
        state.text.language()
    });
    let indentation = create_memo(move || {
        content.get();
        state.text.indentation()
    });
    let language_index = create_memo(move || {
        TextLanguage::ALL
            .iter()
            .position(|choice| *choice == language.get())
    });
    let indentation_index = create_memo(move || {
        Some(usize::from(matches!(
            indentation.get(),
            TextIndentation::Spaces { .. }
        )))
    });
    let spaces = create_memo(move || matches!(indentation.get(), TextIndentation::Spaces { .. }));
    let width = create_memo(move || match indentation.get() {
        TextIndentation::Spaces { width } => f64::from(width),
        TextIndentation::Tabs => 2.0,
    });
    view! {
        <List spacing=SETTINGS_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <Text @sizing=ItemSize::Percent(100.0) string="Language" />
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
                            state.text.execute(EditorCommand::SetLanguage(*choice));
                        }}
                    />
                </Frame>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <Text @sizing=ItemSize::Percent(100.0) string="Indentation" />
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
                            state.text.execute(EditorCommand::SetIndentation(indentation));
                        }}
                    />
                </Frame>
            </List>
            <Show condition={spaces}>
                {move || view! {
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=SETTINGS_SPACING
                    >
                        <Text @sizing=ItemSize::Percent(100.0) string="Indentation width" />
                        <Frame width=NUMBER_WIDTH>
                            <NumberInput
                                label="Indentation width"
                                value={width}
                                min=1.0
                                max=8.0
                                @test_id={"text.indentation-width"}
                                on_change={move |value: f64| {
                                    let width = value.round().clamp(1.0, 8.0) as u8;
                                    state.text.execute(EditorCommand::SetIndentation(
                                        TextIndentation::Spaces { width },
                                    ));
                                }}
                            />
                        </Frame>
                    </List>
                }}
            </Show>
        </List>
    }
}

fn pick_block(state: State) {
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
            if !state.is_alive() {
                return;
            }
            state
                .inner()
                .client
                .set_parent(block.id, BlockParent::Block(state.block_id));
            let types = state.host().block_types();
            let name = match state.inner().client.info(block.id) {
                Some(info) => info.label(types.as_ref()).name,
                None => BlockLabel::new(types.as_ref(), block.block_type, None, false).name,
            };
            state.insert_image_embed(block.id, &name);
        },
    );
}
