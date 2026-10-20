use block_editor_beui::be_block::input_settings::RemapperChoice;
use block_editor_beui::be_block::{InputRemapper, InputRemapperContent, InputSettings, Root};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{BlockParent, Editor};
use uuid::Uuid;

use super::ui::Settings;

#[component]
pub(super) fn RemapperSetting(
    editor: Editor,
    settings: Settings,
    read_only: Memo<bool>,
) -> NodeId {
    let effective = settings.effective();
    let remapper = InputSettings::remapper_setting();
    let set_remapper = settings.setter(&remapper);
    let own = settings.value(&remapper);
    let row = settings.row(&remapper);
    let remappers = create_memo(move || {
        editor
            .block_types()
            .editors_for(InputRemapper::CONTENT_TYPE)
            .into_iter()
            .filter(|entry| entry.block_type == Some(InputRemapper::CONTENT_TYPE))
            .map(|entry| (entry.editor, entry.name.clone()))
            .collect::<Vec<(Uuid, String)>>()
    });
    let keys = create_memo(move || {
        remappers.with(|remappers| remappers.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>())
    });
    let chosen = create_memo(move || effective.get().remapper);
    let selected = create_memo(move || {
        chosen.get().and_then(|choice| {
            keys.with(|keys| keys.iter().position(|editor| *editor == choice.editor))
        })
    });
    let set_here = row.modified();
    let none = create_memo(move || keys.with(Vec::is_empty));
    let link_off = create_memo(move || none.get() || (read_only.get() && chosen.get().is_none()));
    let detail = create_memo(move || {
        let name = |editor: Uuid| {
            remappers.with(|remappers| {
                remappers
                    .iter()
                    .find(|(id, _)| *id == editor)
                    .map(|(_, name)| name.clone())
            })
        };
        match chosen.get() {
            Some(choice) => format!("Edited with {}.", name(choice.editor).unwrap_or_default()),
            None if none.get() => "No plugin here remaps input.".to_owned(),
            None => "None yet: opening them makes them.".to_owned(),
        }
    });
    let create = move |remapper: Uuid| {
        let name = remappers.with_untracked(|remappers| {
            remappers
                .iter()
                .find(|(id, _)| *id == remapper)
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        });
        let parent = settings.block_for(&InputSettings::remapper_setting())?;
        let block = editor.blocks().create_named(
            &InputRemapperContent::default(),
            BlockParent::Block(parent),
            name,
        );
        let choice = RemapperChoice {
            block,
            editor: remapper,
        };
        set_remapper.call(Some(choice));
        Some(choice)
    };
    let choose = move |index: Option<usize>| {
        let Some(remapper) =
            index.and_then(|index| keys.with_untracked(|keys| keys.get(index).copied()))
        else {
            return;
        };
        if chosen.get_untracked().map(|choice| choice.editor) != Some(remapper) {
            create(remapper);
        }
    };
    let open = move || {
        let choice = match chosen.get_untracked() {
            Some(choice) => choice,
            None => match keys.with_untracked(|keys| keys.first().copied()) {
                Some(remapper) if !read_only.get_untracked() => match create(remapper) {
                    Some(choice) => choice,
                    None => return,
                },
                _ => return,
            },
        };
        editor
            .host()
            .open_block_with(choice.block, InputRemapper::CONTENT_TYPE, choice.editor);
    };
    let reset = move || {
        let Some(choice) = own.get_untracked() else {
            return;
        };
        if let Some(parent) = settings.block_for(&InputSettings::remapper_setting()) {
            editor.host().delete_block(
                choice.block,
                InputRemapper::CONTENT_TYPE,
                BlockParent::Block(parent),
                false,
            );
        }
        set_remapper.call(None);
    };
    view! {
        <SettingsSection title="Remaps">
            <SettingsRow
                label="Remapper"
                description="The plugin that changes keys, buttons and wheel turns in a Linux session."
                modified={set_here}
                locked={read_only}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={keys}>|remapper: Uuid| {
                            let label = create_memo(move || {
                                remappers.with(|remappers| {
                                    remappers
                                        .iter()
                                        .find(|(id, _)| *id == remapper)
                                        .map(|(_, name)| name.clone())
                                        .unwrap_or_default()
                                })
                            });
                            view! {
                                <ChoiceOption label={label} />
                            }
                        }</ForEach>
                    }}
                    selected={selected}
                    label="Remapper"
                    disabled={read_only}
                    @test_id={"input.remapper"}
                    on_change={choose}
                />
            </SettingsRow>
            <SettingsLink
                label="Remaps"
                description={detail}
                separated=true
                disabled={link_off}
                modified={set_here}
                locked={read_only}
                reset_test_id={"input.remaps.reset".to_owned()}
                reset_confirm="Resetting deletes the remaps made here, and this device follows the ones its other settings pick, if any."
                @test_id={"input.remaps"}
                on_reset={reset}
                on_click={open}
            />
        </SettingsSection>
    }
}
