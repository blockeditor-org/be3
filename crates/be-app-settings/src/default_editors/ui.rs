use crate::custom::unlisted;
use be_sdk_beui::be_sdk_ui::BlockTypes;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{ChangesToast, Editor, LayeredSettings, LayeredSettingsRow};
use be_type_settings::EditorSettings;
use beui::Extent;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
struct Choices {
    name: String,
    editors: Vec<(Uuid, String)>,
}

#[component]
pub fn DefaultEditorsView(editor: Editor, settings: LayeredSettings<EditorSettings>) -> NodeId {
    let known = editor.block_types_known();
    let types = create_memo(move || {
        known.track();
        let catalog = editor.block_types();
        let mut types: Vec<(String, Uuid)> = catalog
            .iter()
            .filter(|(block_type, _)| catalog.editors_for(**block_type, None).len() > 1)
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
    let page = NodeRef::new();
    view! {
        <SettingsPage @node_ref=&page>
            <SettingsSection
                title="Default editors"
                description="The editor a block of each type opens in. Open with, in Files, opens one in another."
            >
                <ForEach
                    value={types}
                    fallback={|| view! {
                        <SettingsRow label="Every block type has one editor." />
                    }}
                >|block_type: Uuid, index: ReadSignal<usize>|
                    <DefaultEditor
                        editor={editor}
                        settings={settings}
                        block_type={block_type}
                        separated={move || index.get() != 0}
                    />
                </ForEach>
            </SettingsSection>
            <ChangesToast changes={settings.changes()} anchor={page} />
        </SettingsPage>
    }
}

#[component]
fn DefaultEditor(
    editor: Editor,
    settings: LayeredSettings<EditorSettings>,
    block_type: Uuid,
    separated: Prop<bool>,
) -> NodeId {
    let choices = create_memo(move || {
        let catalog = editor.block_types();
        Choices {
            name: catalog
                .display_name(block_type)
                .map_or_else(|| block_type.to_string(), str::to_owned),
            editors: catalog
                .editors_for(block_type, None)
                .into_iter()
                .map(|entry| (entry.editor, entry.name.clone()))
                .collect(),
        }
    });
    let name = choices.with_untracked(|choices| choices.name.clone());
    let setting = EditorSettings::default_editor_setting(block_type, name);
    let chosen = settings.value(&setting);
    let below = settings.fallback(&setting);
    let set = settings.setter(&setting);
    let row = settings.row(&setting);
    let inherited_label = move || {
        let wanted = below.get();
        let name = choices.with(|choices| {
            wanted
                .and_then(|wanted| choices.editors.iter().find(|(id, _)| *id == wanted))
                .or_else(|| choices.editors.first())
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        });
        format!("Default ({name})")
    };
    let ids = create_memo(move || {
        choices.with(|choices| {
            choices
                .editors
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<Uuid>>()
        })
    });
    let selected = create_memo(move || Some(chosen.get()));
    let custom = unlisted(chosen, move |id| ids.with(|ids| ids.contains(id)));
    let name = create_memo(move || choices.with(|choices| choices.name.clone()));
    let read_only = editor.read_only();
    view! {
        <LayeredSettingsRow
            setting={row}
            label={name}
            separated={separated}
            locked={read_only}
            id={format!("default-editors.{block_type}")}
        >
            <ListItem grow=1.0>
                <Select
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption value=None label={inherited_label} />
                        <ForEach value={ids}>|id: Uuid| {
                            let label = create_memo(move || {
                                choices.with(|choices| {
                                    choices
                                        .editors
                                        .iter()
                                        .find(|(known, _)| *known == id)
                                        .map(|(_, name)| name.clone())
                                        .unwrap_or_default()
                                })
                            });
                            view! {
                                <ChoiceOption value={Some(id)} label={label} />
                            }
                        }</ForEach>
                        <ForEach value={custom}>|id: Uuid|
                            <ChoiceOption value={Some(id)} label="Unknown editor" />
                        </ForEach>
                    }}
                    selected={Binding::new(selected, move |chosen: Option<Option<Uuid>>| {
                        if let Some(chosen) = chosen {
                            set.call(chosen);
                        }
                    })}
                    label={name}
                    disabled={read_only}
                    @test_id={format!("default-editors.{block_type}")}
                />
            </ListItem>
        </LayeredSettingsRow>
    }
}
