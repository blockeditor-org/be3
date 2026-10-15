use block_editor_beui::Editor;
use block_editor_beui::be_block::{EditorSettings, EditorSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Scroll, Select, use_theme};
use block_editor_beui::beui::styled::{Text, TextStyle};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::block_ui::BlockTypes;
use uuid::Uuid;

const PADDING: f32 = 20.0;
const SPACING: f32 = 8.0;

#[derive(Clone, PartialEq)]
struct Choices {
    name: String,
    editors: Vec<(Uuid, String)>,
}

#[component]
pub fn DefaultEditorsView(editor: Editor) -> NodeId {
    let types = create_memo(move || {
        let catalog = editor.block_types();
        let mut types: Vec<(String, Uuid)> = catalog
            .iter()
            .filter(|(block_type, _)| catalog.editors_for(**block_type).len() > 1)
            .map(|(block_type, _)| {
                let name = catalog
                    .display_name(*block_type)
                    .map_or_else(|| block_type.to_string(), str::to_owned);
                (name, *block_type)
            })
            .collect();
        types.sort();
        types
            .into_iter()
            .map(|(_, block_type)| block_type)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || types.with(Vec::is_empty));
    let theme = use_theme();
    view! {
        <Frame color={theme.background}>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                        <List spacing=SPACING>
                            <Text style=TextStyle::HEADING string="Default editors" />
                            <Text
                                style=TextStyle::CAPTION
                                string="The editor a block of each type opens in. Open with, in Files, opens one in another."
                            />
                            <Show condition={empty}>
                                <Text
                                    style=TextStyle::CAPTION
                                    string="Every block type has one editor."
                                />
                            </Show>
                            <ForEach keys={types}>
                                {move |block_type: Uuid| view! {
                                    <DefaultEditor editor={editor} block_type={block_type} />
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn DefaultEditor(editor: Editor, block_type: Uuid) -> NodeId {
    let settings = editor.block_content::<EditorSettingsContent>();
    let chosen = settings.project(move |content| content.root().default_editor(block_type));
    let inherited = editor.inherited_settings::<EditorSettings>();
    let choices = create_memo(move || {
        let catalog = editor.block_types();
        Choices {
            name: catalog
                .display_name(block_type)
                .map_or_else(|| block_type.to_string(), str::to_owned),
            editors: catalog
                .editors_for(block_type)
                .into_iter()
                .map(|entry| (entry.editor, entry.name.clone()))
                .collect(),
        }
    });
    let inherited_label = create_memo(move || {
        let wanted = inherited.with(|inherited| inherited.default_editor(block_type));
        let name = choices.with(|choices| {
            wanted
                .and_then(|wanted| choices.editors.iter().find(|(id, _)| *id == wanted))
                .or_else(|| choices.editors.first())
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        });
        format!("Inherited ({name})")
    });
    let names = create_memo(move || {
        choices.with(|choices| (0..choices.editors.len()).collect::<Vec<usize>>())
    });
    let selected = create_memo(move || {
        Some(chosen.get().map_or(0, |chosen| {
            choices.with(|choices| {
                choices
                    .editors
                    .iter()
                    .position(|(id, _)| *id == chosen)
                    .map_or(0, |index| index + 1)
            })
        }))
    });
    let name = create_memo(move || choices.with(|choices| choices.name.clone()));
    let read_only = editor.read_only();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Text @sizing=ItemSize::Percent(100.0) string={name} />
            <Select
                options={view! {
                    <ChoiceOption label={inherited_label} />
                    <ForEach keys={names}>
                        {move |index: usize| {
                            let label = create_memo(move || {
                                choices.with(|choices| {
                                    choices
                                        .editors
                                        .get(index)
                                        .map(|(_, name)| name.clone())
                                        .unwrap_or_default()
                                })
                            });
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }}
                    </ForEach>
                }}
                selected={selected}
                label={name}
                disabled={read_only}
                @test_id={format!("default-editors.{block_type}")}
                on_change={move |index: Option<usize>| {
                    let chosen = index.and_then(|index| index.checked_sub(1)).and_then(|index| {
                        choices.with_untracked(|choices| choices.editors.get(index).map(|(id, _)| *id))
                    });
                    settings.operate(EditorSettings::set_default_editor(block_type, chosen));
                }}
            />
        </List>
    }
}
