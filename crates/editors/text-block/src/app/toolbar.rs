use beui::Key;
use beui::NodeId;
use beui::icons::{
    ICON_BOLD, ICON_BRACKETS, ICON_CODE, ICON_HEADING, ICON_IMAGE, ICON_ITALIC, ICON_KEYBOARD_OFF,
    ICON_LINK, ICON_LIST, ICON_LIST_CHECKS, ICON_LIST_ORDERED, ICON_SETTINGS, ICON_SQUARE_PLUS,
    ICON_STRIKETHROUGH,
};
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::BlockParent;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::{BlockFilter, block_ui::BlockLabel};
use text_editor_core::{EditorCommand, MarkdownCommand, TextIndentation, TextLanguage};

use super::state::State;

const TOOLBAR_SPACING: Space = Space::Sm;
const SETTINGS_SPACING: Space = Space::Md;
const SELECT_WIDTH: Size = Size::Custom(120.0);
const NUMBER_WIDTH: Size = Size::Custom(70.0);
const FORMAT_PADDING: Space = Space::Sm;

#[component]
pub(crate) fn EditorMenu(state: State) -> NodeId {
    let settings = create_signal(false);
    menu_actions(state, settings);
    let hex = state.hex_view;
    let text_view = create_memo(move || !hex.get());
    let hex_view = create_memo(move || hex.get());
    view! {
        <Dialog open={settings} title="Text settings" on_dismiss={move || settings.set(false)}>
            <List spacing=SETTINGS_SPACING>
                <Show condition={text_view}>
                    <TextSettings state={state} />
                </Show>
                <Show condition={hex_view}>
                    <HexSettings state={state} />
                </Show>
            </List>
        </Dialog>
    }
}

fn menu_actions(state: State, settings: Signal<bool>) {
    let hex = state.hex_view;
    let in_text = create_memo(move || !hex.get());
    let hex_view = Action::new("text.hex", "Hex view", move || state.toggle_hex_view())
        .glyph(ICON_BRACKETS)
        .checked(create_memo(move || hex.get()))
        .register();
    let insert = Action::new("text.insert-block", "Insert a block", move || {
        pick_block(state)
    })
    .glyph(ICON_SQUARE_PLUS)
    .enabled(in_text)
    .register();
    let settings = Action::new("text.settings", "Text settings", move || settings.set(true))
        .glyph(ICON_SETTINGS)
        .register();
    add_to_menu(vec![
        MenuNode::action(&hex_view),
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
            .glyph(*glyph)
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
    let typing = state.typing;
    let hex = state.hex_view;
    let row = NodeRef::new();
    let held = holds_focus(row);
    arrow_focus(row, Direction::Horizontal);
    let shown = create_memo(move || (typing.get() || held.get()) && !hex.get());
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
        view! {
            <Interactive press_focus=false @node_ref=&row @test_id={"text.format"}>
                <Frame fill=ColorRole::Surface>
                    <List spacing=Space::None>
                        <Separator />
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=Space::None
                        >
                            <Scroll
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                            >
                                <Frame padding=FORMAT_PADDING>
                                    <List spacing=Space::None>
                                        <Show condition={markdown} then={controls} />
                                    </List>
                                </Frame>
                            </Scroll>
                            <Show condition={keyboard}>
                                <Frame padding=FORMAT_PADDING>
                                    <Button
                                        icon_only=true
                                        glyph=ICON_KEYBOARD_OFF
                                        label="Done"
                                        @test_id={"text.format.done"}
                                        on_click={|| with_document(|document| document.update_focus(None))}
                                    />
                                </Frame>
                            </Show>
                        </List>
                    </List>
                </Frame>
            </Interactive>
        }
    };
    view! {
        <List spacing=Space::None>
            <Show condition={shown} then={bar} />
        </List>
    }
}

#[component]
fn MarkdownControls(formats: Vec<Action>) -> NodeId {
    let count = formats.len();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <ForEach keys={(0..count).collect::<Vec<usize>>()}>|index: usize| {
                let action = formats[index].clone();
                let id = action.id().to_owned();
                view! {
                    <Button icon_only=true @test_id={id} action />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn HexSettings(state: State) -> NodeId {
    let insert = state.hex_insert_mode;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
            <Text @sizing=ItemSize::Percent(100.0)>"Insert instead of overwriting"</Text>
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
                <Text @sizing=ItemSize::Percent(100.0)>"Language"</Text>
                <Select
                    width=SELECT_WIDTH
                    label="Language"
                    selected={Binding::new(language_index, move |index: Option<usize>| {
                        let Some(choice) = index.and_then(|index| TextLanguage::ALL.get(index)) else {
                            return;
                        };
                        state.text.execute(EditorCommand::SetLanguage(*choice));
                    })}
                    @test_id={"text.language"}
                    options={view! {
                        <ForEach
                            keys={(0..TextLanguage::ALL.len()).collect::<Vec<usize>>()}
                        >|index: usize|
                            <ChoiceOption label={TextLanguage::ALL[index].label().to_owned()} />
                        </ForEach>
                    }}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                <Text @sizing=ItemSize::Percent(100.0)>"Indentation"</Text>
                <Select
                    width=SELECT_WIDTH
                    label="Indentation"
                    selected={Binding::new(indentation_index, move |index: Option<usize>| {
                        let indentation = match index {
                            Some(1) => TextIndentation::Spaces { width: 2 },
                            _ => TextIndentation::Tabs,
                        };
                        state.text.execute(EditorCommand::SetIndentation(indentation));
                    })}
                    @test_id={"text.indentation"}
                    options={view! {
                        <ChoiceOption label="Tabs" />
                        <ChoiceOption label="Spaces" />
                    }}
                />
            </List>
            <Show condition={spaces}>
                <List direction=Direction::Horizontal align=Align::Center spacing=SETTINGS_SPACING>
                    <Text @sizing=ItemSize::Percent(100.0)>"Indentation width"</Text>
                    <Frame width=NUMBER_WIDTH>
                        <NumberInput
                            label="Indentation width"
                            value={Binding::new(width, move |value: f64| {
                                let width = value.round().clamp(1.0, 8.0) as u8;
                                state.text.execute(EditorCommand::SetIndentation(
                                    TextIndentation::Spaces { width },
                                ));
                            })}
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
