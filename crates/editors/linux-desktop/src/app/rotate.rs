use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::be_block::display_settings::Rotation;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_ROTATE_CW_SQUARE;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::PopoverHandle;
use block_editor_beui::{Editor, HostDisplay, LayeredSettings, RotationPicker};

use super::popup::BarPopup;

const PICKER_WIDTH: f32 = 268.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn RotateButton(editor: Editor, display: Memo<Option<HostDisplay>>) -> NodeId {
    let settings = LayeredSettings::<DisplaySettings>::this_device(editor);
    let effective = settings.effective();
    let ready = settings.ready();
    let waiting = create_memo(move || !ready.get());
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
        settings.set(&DisplaySettings::rotation_setting(&id), Some(rotation));
    };
    view! {
        <BarPopup
            label="Rotate the screen"
            glyph=ICON_ROTATE_CW_SQUARE
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
                    <Text>"Tap the one that's right side up"</Text>
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
