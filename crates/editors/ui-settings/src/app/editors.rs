use block_editor_beui::Editor;
use block_editor_beui::be_block::{EditorSettings, EditorSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::block_ui::BlockTypes;
use uuid::Uuid;

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
    view! {
        <SettingsPage>
            <SettingsSection
                title="Default editors"
                description="The editor a block of each type opens in. Open with, in Files, opens one in another."
            >
                <Show condition={empty}>
                    <SettingsRow label="Every block type has one editor." />
                </Show>
                <ForEach keys={types}>
                    {move |block_type: Uuid| {
                        let separated = create_memo(move || {
                            types.with(|types| types.first() != Some(&block_type))
                        });
                        view! {
                            <DefaultEditor
                                editor={editor}
                                block_type={block_type}
                                separated={separated}
                            />
                        }
                    }}
                </ForEach>
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn DefaultEditor(editor: Editor, block_type: Uuid, separated: Memo<bool>) -> NodeId {
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
        format!("Default ({name})")
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
    let modified = create_memo(move || chosen.get().is_some());
    view! {
        <SettingsRow
            label={name}
            separated={separated}
            modified={modified}
            locked={read_only}
            reset_test_id={format!("default-editors.{block_type}.reset")}
            on_reset={move || settings.operate(EditorSettings::set_default_editor(block_type, None))}
        >
            <Select
                @sizing=ItemSize::Percent(100.0)
                width={Extent::fraction(1.0)}
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
        </SettingsRow>
    }
}
