use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_VOLUME_X;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::desktop;
use block_editor_beui::{Editor, MediaLevel, MediaLevels, MediaRequest};

use super::media::volume_glyph;
use super::popup::BarPopup;

const SPACING: Space = Space::Lg;
const LABEL_SPACING: Space = Space::Xs;
const PERCENT_WIDTH: f32 = 44.0;

fn percent(level: MediaLevel) -> String {
    format!("{}%", (level.level * 100.0).round())
}

#[component]
pub(crate) fn VolumeButton(editor: Editor, levels: Memo<MediaLevels>) -> NodeId {
    let output = create_memo(move || levels.with(|levels| levels.output));
    let glyph = create_memo(move || output.get().map_or(ICON_VOLUME_X, volume_glyph).to_owned());
    let label = create_memo(move || output.get().map(percent).unwrap_or_default());
    view! {
        <BarPopup label glyph @test_id={"desktop.volume"}>|_: PopoverHandle|
            <VolumePanel editor={editor} levels={levels} />
        </BarPopup>
    }
}

#[component]
fn VolumePanel(editor: Editor, levels: Memo<MediaLevels>) -> NodeId {
    let output = create_memo(move || levels.with(|levels| levels.output));
    let volume = create_memo(move || output.get().map_or(0.0, |output| output.level));
    let shown = create_memo(move || output.get().map(percent).unwrap_or_default());
    let muted = create_memo(move || output.get().is_some_and(|output| output.muted));
    let listed = create_memo(move || !levels.with(|levels| levels.outputs.is_empty()));
    view! {
        <List spacing=SPACING @test_id={"desktop.volume.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Slider
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(volume, move |level: f32| editor.act(move || desktop::media::request(MediaRequest::SetVolume(level))))}
                    label="Volume"
                    @test_id={"desktop.volume.level"}
                />
                <Text @sizing=ItemSize::Fixed(PERCENT_WIDTH)>{shown}</Text>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text @sizing=ItemSize::Percent(100.0)>"Mute"</Text>
                <Switch
                    on={Binding::new(muted, move |mute: bool| editor.act(move || desktop::media::request(MediaRequest::SetMute(mute))))}
                    label="Mute"
                    @test_id={"desktop.volume.mute"}
                />
            </List>
            <Show condition={listed}>
                <OutputPicker editor={editor} levels={levels} />
            </Show>
        </List>
    }
}

#[component]
fn OutputPicker(editor: Editor, levels: Memo<MediaLevels>) -> NodeId {
    let outputs = create_memo(move || levels.with(|levels| levels.outputs.clone()));
    let keys = create_memo(move || {
        outputs.with(|outputs| {
            outputs
                .iter()
                .map(|output| output.id.clone())
                .collect::<Vec<_>>()
        })
    });
    let selected = create_memo(move || levels.with(|levels| levels.default_output.clone()));
    let choose = move |chosen: Option<String>| {
        if let Some(id) = chosen {
            editor.act(move || desktop::media::request(MediaRequest::ChooseOutput(id)));
        }
    };
    view! {
        <List spacing=LABEL_SPACING>
            <Text style=TextStyle::CAPTION>"Output"</Text>
            <Select
                options={view! {
                    <ForEach keys={keys}>|id: String| {
                        let value = id.clone();
                        let label = create_memo(move || {
                            outputs.with(|outputs| {
                                outputs
                                    .iter()
                                    .find(|output| output.id == id)
                                    .map(|output| output.name.clone())
                                    .unwrap_or_default()
                            })
                        });
                        view! {
                            <ChoiceOption value label />
                        }
                    }</ForEach>
                }}
                selected={Binding::new(selected, choose)}
                label="Output"
                @test_id={"desktop.volume.output"}
            />
        </List>
    }
}
