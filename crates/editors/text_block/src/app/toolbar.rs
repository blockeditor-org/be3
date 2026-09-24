use beui::NodeId;
use beui::icons::{
    ICON_CHECKLIST, ICON_CODE, ICON_DATA_ARRAY, ICON_FIND_REPLACE, ICON_FORMAT_BOLD,
    ICON_FORMAT_ITALIC, ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED,
    ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE, ICON_LINK, ICON_TITLE,
};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, clone, component, create_memo, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, IconButton, MenuButton, NumberInput, Select, ToggleButton,
    use_theme,
};
use beui::unstyled::{ChoiceOption, MenuItem, Scroll};
use block::BlockParent;
use block_editor_plugin::{BlockFilter, Toolbar, block_ui::BlockLabel};
use text_editor_core::{EditorCommand, MarkdownCommand, TextIndentation, TextLanguage};

use super::state::Shared;

const TOOLBAR_SPACING: f32 = 6.0;
const SELECT_WIDTH: f32 = 120.0;
const NUMBER_WIDTH: f32 = 70.0;

#[component]
pub(crate) fn EditorToolbar(state: Shared) -> NodeId {
    let hex = state.hex_view.clone();
    let text_view = create_memo(clone!(hex -> move || !hex.get()));
    let hex_view = create_memo(clone!(hex -> move || hex.get()));
    let toggle = state.clone();
    let hex_label = create_memo(clone!(hex -> move || match hex.get() {
        true => "Switch to text view".to_owned(),
        false => "Switch to hex view".to_owned(),
    }));
    let hex_state = state.clone();
    view! {
        <Toolbar>
            <ToggleButton
                glyph=ICON_DATA_ARRAY
                icon_only=true
                label={hex_label}
                pressed={hex}
                @test_id={"text.hex-view"}
                on_change={move |_| toggle.toggle_hex_view()}
            />
            <Show condition={hex_view}>
                {move || view! {
                    <HexControls state={hex_state.clone()} />
                }}
            </Show>
            <Show condition={text_view}>
                {move || view! {
                    <TextControls @sizing=ItemSize::Percent(100.0) state={state.clone()} />
                }}
            </Show>
        </Toolbar>
    }
}

#[component]
fn HexControls(state: Shared) -> NodeId {
    let insert = state.hex_insert_mode.clone();
    view! {
        <ToggleButton
            label="Insert"
            pressed={insert}
            @test_id={"text.hex-insert"}
            on_change={move |pressed: bool| {
                state.set_hex_insert_mode.set(pressed);
                state.hex_pending_nibble.set(None);
            }}
        />
    }
}

#[component]
fn TextControls(state: Shared) -> NodeId {
    let theme = use_theme();
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
    let markdown = create_memo(clone!(language -> move || {
        language.get() == TextLanguage::Markdown
    }));
    let language_state = state.clone();
    let indentation_state = state.clone();
    let width_state = state.clone();
    let markdown_state = state.clone();
    let insert_state = state.clone();
    view! {
        <Scroll direction=Direction::Horizontal focus_color={theme.accent.clone()}>
            <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
                <Body content="Language:" />
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
                <Body content="Indentation:" />
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
                <Show condition={spaces}>
                    {move || view! {
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
                    }}
                </Show>
                <Show condition={markdown}>
                    {move || view! {
                        <MarkdownControls state={markdown_state.clone()} />
                    }}
                </Show>
                <Button
                    label="Insert"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.insert-block"}
                    on_click={move || pick_block(&insert_state)}
                />
                <IconButton
                    glyph=ICON_FIND_REPLACE
                    label="Find and replace"
                    @test_id={"text.find-replace"}
                    on_click={clone!(state -> move || state.text.open_find(true))}
                />
            </List>
        </Scroll>
    }
}

#[component]
fn MarkdownControls(state: Shared) -> NodeId {
    let theme = use_theme();
    let heading_state = state.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_FORMAT_BOLD
                label="Bold"
                id="text.bold"
                command=MarkdownCommand::Bold
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_FORMAT_ITALIC
                label="Italic"
                id="text.italic"
                command=MarkdownCommand::Italic
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_FORMAT_STRIKETHROUGH
                label="Strikethrough"
                id="text.strikethrough"
                command=MarkdownCommand::Strikethrough
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_CODE
                label="Inline code"
                id="text.inline-code"
                command=MarkdownCommand::InlineCode
            />
            <MenuButton
                glyph=ICON_TITLE
                icon_only=true
                label="Heading"
                @test_id={"text.heading"}
                items={view! {
                    <ForEach keys={(1..=6_usize).collect::<Vec<usize>>()}>
                        {move |level: usize| view! {
                            <MenuItem label={format!("Heading {level}")} />
                        }}
                    </ForEach>
                }}
                on_select={move |path: Vec<usize>| {
                    let Some(index) = path.first() else {
                        return;
                    };
                    heading_state.text.execute(EditorCommand::Markdown(MarkdownCommand::Heading(
                        *index as u8 + 1,
                    )));
                }}
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_FORMAT_LIST_BULLETED
                label="Bulleted list"
                id="text.bulleted-list"
                command=MarkdownCommand::BulletedList
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_FORMAT_LIST_NUMBERED
                label="Numbered list"
                id="text.numbered-list"
                command=MarkdownCommand::NumberedList
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_CHECKLIST
                label="Checklist"
                id="text.checklist"
                command=MarkdownCommand::Checklist
            />
            <MarkdownButton
                state={state.clone()}
                glyph=ICON_LINK
                label="Link"
                id="text.link"
                command=MarkdownCommand::Link
            />
            <MarkdownButton
                state={state}
                glyph=ICON_IMAGE
                label="Image"
                id="text.image"
                command=MarkdownCommand::Image
            />
            <Frame width=1.0 height=18.0 color={theme.border.clone()} />
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
) -> NodeId {
    view! {
        <IconButton
            glyph={glyph}
            label={label}
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
        },
        move |result| {
            let Ok(block) = result else {
                return;
            };
            picked
                .client
                .set_block_parent(block.id, BlockParent::Uuid(picked.block_id));
            let types = picked.host().block_types();
            let name = match picked.client.cached_block(block.id) {
                Some(cached) => BlockLabel::for_cached(types.as_ref(), &cached).name,
                None => BlockLabel::new(types.as_ref(), block.block_type, None).name,
            };
            picked.insert_image_embed(block.id, &name);
        },
    );
}
