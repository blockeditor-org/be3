use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::be_block::display_settings::Rotation;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_ROTATE_CW_SQUARE;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Editor, HostDisplay, LayeredSettings, RotationPicker};

use super::popup::BarPopup;

const SPACING: Space = Space::Md;

#[component]
pub(crate) fn RotateButton(editor: Editor, display: Memo<Option<HostDisplay>>) -> NodeId {
    view! {
        <BarPopup
            label="Rotate the screen"
            glyph=ICON_ROTATE_CW_SQUARE
            icon_only=true
            @test_id={"desktop.rotate"}
        >|handle: PopoverHandle| {
            let picked = Callback::new(move |()| handle.close.call(()));
            view! {
                <ScreenRotation
                    editor={editor}
                    display={display}
                    test_id={"desktop.rotate".to_owned()}
                    on_picked={picked}
                />
            }
        }</BarPopup>
    }
}

#[component]
pub(crate) fn ScreenRotation(
    editor: Editor,
    display: Memo<Option<HostDisplay>>,
    test_id: String,
    on_picked: Callback<()>,
) -> NodeId {
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
    let pick = Callback::new(move |rotation: Rotation| {
        let Some(id) = display.with_untracked(|display| display.as_ref().map(|d| d.id.clone()))
        else {
            return;
        };
        settings.set(&DisplaySettings::rotation_setting(&id), Some(rotation));
        on_picked.call(());
    });
    view! {
        <List spacing=SPACING>
            <Text>"Tap the one that's right side up"</Text>
            <RotationPicker
                test_id={test_id}
                display={display}
                chosen={chosen}
                read_only={waiting}
                on_pick={pick}
            />
        </List>
    }
}
