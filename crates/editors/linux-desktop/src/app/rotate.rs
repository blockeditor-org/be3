use block_editor_beui::be_block::display_settings::Rotation;
use block_editor_beui::be_block::{DisplaySettings, DisplaySettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_SCREEN_ROTATION;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::PopoverHandle;
use block_editor_beui::root_settings::RootSetting;
use block_editor_beui::{Editor, HostDisplay, RotationPicker};

use super::popup::BarPopup;

const PICKER_WIDTH: f32 = 268.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn RotateButton(editor: Editor, display: Memo<Option<HostDisplay>>) -> NodeId {
    let client = editor.host().client_id();
    let effective = editor.effective_settings::<DisplaySettings>();
    let top =
        create_memo(move || RootSetting::<DisplaySettingsContent>::default().find(&editor, client));
    let settled = editor
        .related_content::<DisplaySettingsContent>(top)
        .settled();
    let waiting = create_memo(move || top.get().is_some() && !settled.get());
    let chosen = create_memo(move || {
        display.with(|display| {
            display.as_ref().map_or(Rotation::Normal, |display| {
                effective.with(|settings| settings.rotation(&display.id))
            })
        })
    });
    let write = move |rotation: Rotation| {
        let Some(id) = display.with_untracked(|display| display.as_ref().map(|d| d.id.clone()))
        else {
            return;
        };
        let block = top
            .get_untracked()
            .or_else(|| RootSetting::<DisplaySettingsContent>::default().ensure(&editor, client));
        let Some(block) = block else {
            return;
        };
        let content = editor.content_of::<DisplaySettingsContent>(block);
        let own = content.read(|content| content.root()).unwrap_or_default();
        content.operate(own.set_rotation(&id, Some(rotation)));
    };
    view! {
        <BarPopup
            label="Rotate the screen"
            glyph={ICON_SCREEN_ROTATION.to_owned()}
            icon_only=true
            width=PICKER_WIDTH
            @test_id={"desktop.rotate"}
        >|handle: PopoverHandle| {
            let pick = Callback::new(move |rotation: Rotation| {
                write(rotation);
                handle.close.call(());
            });
            view! {
                <List spacing=SPACING>
                    <Text string="Tap the one that's right side up" wrap=true />
                    <RotationPicker
                        test_id={"desktop.rotate".to_owned()}
                        display={display}
                        chosen={chosen}
                        read_only={waiting}
                        on_pick={pick}
                    />
                </List>
            }
        }</BarPopup>
    }
}
