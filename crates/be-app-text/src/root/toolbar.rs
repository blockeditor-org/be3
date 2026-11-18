use be_sdk_beui::BlockParent;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::{BlockFilter, be_sdk_ui::BlockLabel};
use beui::Key;
use beui::NodeId;
use beui::icons::{
    ICON_BOLD, ICON_BRACKETS, ICON_CODE, ICON_HASH, ICON_HEADING, ICON_IMAGE, ICON_ITALIC,
    ICON_KEYBOARD_OFF, ICON_LINK, ICON_LIST, ICON_LIST_CHECKS, ICON_LIST_ORDERED, ICON_SETTINGS,
    ICON_SQUARE_PLUS, ICON_STRIKETHROUGH,
};
use beui::reactive::*;
use beui::styled::*;
use beui_text_editor_core::{EditorCommand, MarkdownCommand, TextIndentation, TextLanguage};

use super::state::State;
use beui::{Align, Direction};

const TOOLBAR_SPACING: Space = Space::Sm;
const SETTINGS_SPACING: Space = Space::Md;
const SELECT_WIDTH: Size = Size::Custom(120.0);
const NUMBER_WIDTH: Size = Size::Custom(70.0);

#[component]
pub(crate) fn EditorMenu(state: State) -> NodeId {
    let tab = use_frame_anchor();
    let settings = create_signal(false);
    menu_actions(state, settings);
    view! {
        <Dialog
            open={settings}
            title="Text settings"
            anchor={move || DialogAnchor::Rect(tab.get())}
            on_dismiss={move || settings.set(false)}
        >
            <Show
                value={state.hex_view}
                fallback={move || view! {
                    <TextSettings state={state} />
                }}
            >
                <HexSettings state={state} />
            </Show>
        </Dialog>
    }
}

fn menu_actions(state: State, settings: Signal<bool>) {
    let hex = state.hex_view;
    let hex_view = Action::new("text.hex", "Hex view", move || state.toggle_hex_view())
        .glyph(ICON_BRACKETS)
        .checked(hex)
        .register();
    let line_numbers = state.line_numbers;
    let numbers = Action::new("text.line-numbers", "Line numbers", move || {
        state.toggle_line_numbers()
    })
    .glyph(ICON_HASH)
    .checked(line_numbers)
    .enabled(move || !hex.get())
    .register();
    let insert = Action::new("text.insert-block", "Insert a block", move || {
        pick_block(state)
    })
    .glyph(ICON_SQUARE_PLUS)
    .enabled(move || !hex.get())
    .register();
    let settings = Action::new("text.settings", "Text settings", move || settings.set(true))
        .glyph(ICON_SETTINGS)
        .register();
    add_to_menu(vec![
        MenuNode::action(&hex_view),
        MenuNode::action(&numbers),
        MenuNode::action(&insert),
        MenuNode::action(&settings),
    ]);
}

const FORMATS: [(&str, &str, Glyph, MarkdownCommand, Option<Chord>); 10] = [
    (
        "text.format.heading",
        "Heading",
        ICON_HEADING,
        MarkdownCommand::Heading(2),
        None,
    ),
    (
        "text.format.bold",
        "Bold",
        ICON_BOLD,
        MarkdownCommand::Bold,
        Some(Chord::ctrl(Key::B)),
    ),
    (
        "text.format.italic",
        "Italic",
        ICON_ITALIC,
        MarkdownCommand::Italic,
        Some(Chord::ctrl(Key::I)),
    ),
    (
        "text.format.bulleted-list",
        "Bulleted list",
        ICON_LIST,
        MarkdownCommand::BulletedList,
        None,
    ),
    (
        "text.format.checklist",
        "Checklist",
        ICON_LIST_CHECKS,
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
        ICON_STRIKETHROUGH,
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
        ICON_LIST_ORDERED,
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
        content.track();
        !hex.get() && state.text.language() == TextLanguage::Markdown
    });
    FORMATS
        .iter()
        .map(|(id, label, glyph, command, chord)| {
            let command = *command;
            let mut action = Action::new(*id, *label, move || {
                state.text.execute(EditorCommand::Markdown(command))
            })
            .glyph(*glyph)
            .enabled(markdown);
            if let Some(chord) = chord {
                action = action.shortcut(*chord);
            }
            action.register()
        })
        .collect()
}

/// The format bar: a tool bar of the editor's dock, beside the text's bottom edge until it is moved
/// (`DockBar`, guides/beui/docking.md). It stays while the text shows, rather than hiding when
/// the text loses the keyboard. A press on it leaves the keyboard in the text.
#[component]
pub(crate) fn FormatBar(state: State, formats: Vec<Action>) -> NodeId {
    let keyboard = state.text.touch_mode();
    let markdown = markdown_shown(state);
    let controls = move || {
        let formats = formats.clone();
        let count = formats.len();
        view! {
            <ForEach value={(0..count).collect::<Vec<usize>>()}>|index: usize| {
                let action = formats[index].clone();
                let id = action.id().to_owned();
                view! {
                    <Button icon_only=true @test_id={id} action />
                }
            }</ForEach>
        }
    };
    view! {
        <Interactive press_focus=false @test_id={"text.format"}>
            <DockBarSurface spacing=TOOLBAR_SPACING>
                <Frame @test_id={"text.format.grip"}>
                    <DockBarGrip />
                </Frame>
                <Show value={keyboard}>
                    <Button
                        icon_only=true
                        glyph=ICON_KEYBOARD_OFF
                        label="Done"
                        @test_id={"text.format.done"}
                        on_click={|| set_focus(None)}
                    />
                </Show>
                <Show value={markdown} then={move |_| controls()} />
            </DockBarSurface>
        </Interactive>
    }
}

/// Whether the format bar has anything to offer: the Markdown formats, or Done to put away an
/// on-screen keyboard.
pub(crate) fn format_bar_shown(state: State) -> Memo<bool> {
    let chrome = state.editor.chrome_shown();
    let hex = state.hex_view;
    let touch = state.text.touch_mode();
    let markdown = markdown_shown(state);
    create_memo(move || chrome.get() && !hex.get() && (markdown.get() || touch.get()))
}

fn markdown_shown(state: State) -> Memo<bool> {
    let content = state.text.content();
    create_memo(move || {
        content.track();
        state.text.language() == TextLanguage::Markdown
    })
}

#[component]
fn HexSettings(state: State) -> NodeId {
    let insert = state.hex_insert_mode;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
            <ListItem grow=1.0>
                <Text>"Insert instead of overwriting"</Text>
            </ListItem>
            <Switch
                label="Insert instead of overwriting"
                on={Binding::new(insert, move |pressed: bool| {
                    state.hex_insert_mode.set(pressed);
                    state.inner().hex_pending_nibble.set(None);
                })}
                @test_id={"text.hex-insert"}
            />
        </List>
    }
}

#[component]
fn TextSettings(state: State) -> NodeId {
    let content = state.text.content();
    let language = create_memo(move || {
        content.track();
        state.text.language()
    });
    let indentation = create_memo(move || {
        content.track();
        state.text.indentation()
    });
    let spaces = create_memo(move || matches!(indentation.get(), TextIndentation::Spaces { .. }));
    let width = create_memo(move || match indentation.get() {
        TextIndentation::Spaces { width } => f64::from(width),
        TextIndentation::Tabs => 2.0,
    });
    // A scrubbed width shows here and is set once, when it is kept.
    let width_held = HeldValue::new(Binding::new(width, move |value: f64| {
        let width = value.round().clamp(1.0, 8.0) as u8;
        state
            .text
            .execute(EditorCommand::SetIndentation(TextIndentation::Spaces { width }));
    }));
    view! {
        <List spacing=SETTINGS_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <ListItem grow=1.0>
                    <Text>"Language"</Text>
                </ListItem>
                <Select
                    width=SELECT_WIDTH
                    label="Language"
                    selected={Binding::new(
                        Prop::dynamic(move || Some(language.get())),
                        move |choice: Option<TextLanguage>| {
                            if let Some(choice) = choice {
                                state.text.execute(EditorCommand::SetLanguage(choice));
                            }
                        },
                    )}
                    @test_id={"text.language"}
                    options={view! {
                        <ForEach value={TextLanguage::ALL.to_vec()}>|choice: TextLanguage|
                            <ChoiceOption value=choice label={choice.label().to_owned()} />
                        </ForEach>
                    }}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <ListItem grow=1.0>
                    <Text>"Indentation"</Text>
                </ListItem>
                <Select
                    width=SELECT_WIDTH
                    label="Indentation"
                    selected={Binding::new(
                        Prop::dynamic(move || Some(spaces.get())),
                        move |spaces: Option<bool>| {
                            let indentation = match spaces {
                                Some(true) => TextIndentation::Spaces { width: 2 },
                                _ => TextIndentation::Tabs,
                            };
                            state.text.execute(EditorCommand::SetIndentation(indentation));
                        },
                    )}
                    @test_id={"text.indentation"}
                    options={view! {
                        <ChoiceOption value=false label="Tabs" />
                        <ChoiceOption value=true label="Spaces" />
                    }}
                />
            </List>
            <Show value={spaces}>
                <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                    <ListItem grow=1.0>
                        <Text>"Indentation width"</Text>
                    </ListItem>
                    <Frame width=NUMBER_WIDTH>
                        <NumberInput
                            label="Indentation width"
                            value={width_held.value}
                            on_scrub={width_held.on_drag}
                            min=1.0
                            max=8.0
                            @test_id={"text.indentation-width"}
                        />
                    </Frame>
                </List>
            </Show>
        </List>
    }
}

fn pick_block(state: State) {
    let caret = state.text.caret_indices().first().copied();
    let anchor = caret.and_then(|caret| state.text.layout().get_untracked().caret_rect(caret));
    state.editor.pick_block(
        BlockFilter {
            name: "Insert".to_owned(),
            block_types: Vec::new(),
            excluded: Vec::new(),
            templates: false,
            place: None,
        },
        anchor,
        move |block| {
            if !state.is_alive() {
                return;
            }
            state
                .inner()
                .client
                .set_parent(block.id(), BlockParent::Block(state.block_id));
            let types = state.host().block_types();
            let name = match state.inner().client.info(block.id()) {
                Some(info) => info.label(types.as_ref()).name,
                None => {
                    BlockLabel::new(
                        types.as_ref(),
                        uuid::Uuid::from_bytes(block.block_type),
                        None,
                        false,
                    )
                    .name
                }
            };
            state.insert_image_embed(block.id(), &name);
        },
    );
}
