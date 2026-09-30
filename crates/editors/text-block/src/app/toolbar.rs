use beui::NodeId;
use beui::icons::{
    ICON_ADD_BOX, ICON_CHECKLIST, ICON_CODE, ICON_DATA_ARRAY, ICON_FIND_REPLACE, ICON_FORMAT_BOLD,
    ICON_FORMAT_ITALIC, ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED,
    ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE, ICON_KEYBOARD_HIDE, ICON_LINK, ICON_SETTINGS,
    ICON_TITLE,
};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, WriteSignal, clone, component,
    create_memo, create_signal, view, with_document,
};
use beui::styled::{Body, Dialog, IconButton, NumberInput, Select, Separator, Switch, use_theme};
use beui::unstyled::{ChoiceOption, Scroll};
use block_editor_beui::BlockParent;
use block_editor_beui::{BlockFilter, bar_item, block_ui::BlockLabel};
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
    bar_items(&state, set_settings.clone());
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

fn bar_items(state: &Shared, set_settings: WriteSignal<bool>) {
    let never = create_memo(|| false);
    let hex = state.hex_view.clone();
    let in_hex = create_memo(move || hex.get());
    bar_item(
        "Toggle hex view",
        ICON_DATA_ARRAY,
        never.clone(),
        clone!(state -> move || state.toggle_hex_view()),
    );
    bar_item(
        "Find and replace",
        ICON_FIND_REPLACE,
        in_hex.clone(),
        clone!(state -> move || state.text.open_find(true)),
    );
    bar_item(
        "Insert a block",
        ICON_ADD_BOX,
        in_hex,
        clone!(state -> move || pick_block(&state)),
    );
    bar_item("Text settings", ICON_SETTINGS, never, move || {
        set_settings.set(true)
    });
}

#[component]
pub(crate) fn FormatBar(state: Shared) -> NodeId {
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
        let state = state.clone();
        view! {
            <MarkdownControls state />
        }
    };
    let bar = move || {
        let controls = controls.clone();
        let keyboard = keyboard.clone();
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
fn MarkdownControls(state: Shared) -> NodeId {
    let buttons: Vec<(&str, &str, &str, MarkdownCommand)> = vec![
        (
            ICON_TITLE,
            "Heading",
            "text.format.heading",
            MarkdownCommand::Heading(2),
        ),
        (
            ICON_FORMAT_BOLD,
            "Bold",
            "text.format.bold",
            MarkdownCommand::Bold,
        ),
        (
            ICON_FORMAT_ITALIC,
            "Italic",
            "text.format.italic",
            MarkdownCommand::Italic,
        ),
        (
            ICON_FORMAT_LIST_BULLETED,
            "Bulleted list",
            "text.format.bulleted-list",
            MarkdownCommand::BulletedList,
        ),
        (
            ICON_CHECKLIST,
            "Checklist",
            "text.format.checklist",
            MarkdownCommand::Checklist,
        ),
        (ICON_LINK, "Link", "text.format.link", MarkdownCommand::Link),
        (
            ICON_FORMAT_STRIKETHROUGH,
            "Strikethrough",
            "text.format.strikethrough",
            MarkdownCommand::Strikethrough,
        ),
        (
            ICON_CODE,
            "Inline code",
            "text.format.inline-code",
            MarkdownCommand::InlineCode,
        ),
        (
            ICON_FORMAT_LIST_NUMBERED,
            "Numbered list",
            "text.format.numbered-list",
            MarkdownCommand::NumberedList,
        ),
        (
            ICON_IMAGE,
            "Image",
            "text.format.image",
            MarkdownCommand::Image,
        ),
    ];
    let count = buttons.len();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <ForEach keys={(0..count).collect::<Vec<usize>>()}>
                {move |index: usize| {
                    let (glyph, label, id, command) = buttons[index];
                    view! {
                        <MarkdownButton
                            state={state.clone()}
                            glyph={glyph.to_owned()}
                            label={label.to_owned()}
                            id={id.to_owned()}
                            command
                            press_focus=false
                        />
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
                {move || view! {
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
                }}
            </Show>
        </List>
    }
}

#[component]
fn MarkdownButton(
    state: Shared,
    glyph: String,
    label: String,
    id: String,
    command: MarkdownCommand,
    #[prop(default = true)] press_focus: bool,
) -> NodeId {
    view! {
        <IconButton
            glyph={glyph}
            label={label}
            press_focus
            @test_id={id}
            on_click={move || state.text.execute(EditorCommand::Markdown(command))}
        />
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
