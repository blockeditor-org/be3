use block_editor_beui::be_block::input_settings::RemapperChoice;
use block_editor_beui::be_block::{InputRemapper, InputRemapperContent, InputSettings, Root};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{BlockParent, Editor};
use uuid::Uuid;

use super::ui::Settings;

const ROW_SPACING: f32 = 8.0;

#[component]
pub(super) fn RemapperSetting(
    editor: Editor,
    settings: Settings,
    root: ReadSignal<InputSettings>,
    effective: Memo<InputSettings>,
    read_only: Memo<bool>,
) -> NodeId {
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
        let index = chosen.get().and_then(|choice| {
            keys.with(|keys| keys.iter().position(|editor| *editor == choice.editor))
        });
        Some(index.map_or(0, |index| index + 1))
    });
    let inherited = create_memo(move || root.get().remapper.is_none() && chosen.get().is_some());
    let none_chosen = create_memo(move || chosen.get().is_none());
    let choose = move |index: Option<usize>| {
        let Some(index) = index else {
            return;
        };
        let Some(remapper) = index
            .checked_sub(1)
            .and_then(|index| keys.with_untracked(|keys| keys.get(index).copied()))
        else {
            settings.operate(InputSettings::set_remapper(None));
            return;
        };
        if root.get_untracked().remapper.map(|choice| choice.editor) == Some(remapper) {
            return;
        }
        let name = remappers.with_untracked(|remappers| {
            remappers
                .iter()
                .find(|(id, _)| *id == remapper)
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        });
        let block = editor.blocks().create_named(
            &InputRemapperContent::default(),
            BlockParent::Block(editor.block_id()),
            name,
        );
        settings.operate(InputSettings::set_remapper(Some(RemapperChoice {
            block,
            editor: remapper,
        })));
        editor
            .host()
            .open_block_with(block, InputRemapper::CONTENT_TYPE, remapper);
    };
    let open = move || {
        if let Some(choice) = chosen.get_untracked() {
            editor
                .host()
                .open_block_with(choice.block, InputRemapper::CONTENT_TYPE, choice.editor);
        }
    };
    view! {
        <List spacing=ROW_SPACING>
            <Text style=TextStyle::HEADING string="Remaps" />
            <Text
                style=TextStyle::CAPTION
                string="A plugin that remaps keys, buttons and wheel turns in a Linux session. Choosing one makes a block of its remaps to edit."
                wrap=true
            />
            <List direction=Direction::Horizontal spacing=ROW_SPACING>
                <Select
                    options={view! {
                        <ChoiceOption label="None" />
                        <ForEach keys={keys}>
                            {move |remapper: Uuid| {
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
                            }}
                        </ForEach>
                    }}
                    selected={selected}
                    label="Remapper"
                    disabled={read_only}
                    @test_id={"input-settings.remapper"}
                    on_change={choose}
                />
                <Button
                    label="Open"
                    variant=ButtonVariant::Secondary
                    disabled={none_chosen}
                    @test_id={"input-settings.remapper.open"}
                    on_click={open}
                />
            </List>
            <Show condition={inherited}>
                <Text style=TextStyle::CAPTION string="Chosen in the settings for every device." />
            </Show>
        </List>
    }
}
