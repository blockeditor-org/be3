use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::{Glyph, ICON_VOLUME_X};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::desktop;
use be_sdk_beui::{Editor, MediaLevel, MediaLevels, MediaRequest};

use super::media::volume_glyph;
use beui::{Align, Direction};

const SPACING: Space = Space::Lg;
const LABEL_SPACING: Space = Space::Xs;
const PERCENT_WIDTH: f32 = 44.0;

pub(crate) fn percent(level: MediaLevel) -> String {
    format!("{}%", (level.level * 100.0).round())
}

pub(crate) fn glyph(output: Option<MediaLevel>) -> Glyph {
    output.map_or(ICON_VOLUME_X, volume_glyph)
}

#[component]
pub(crate) fn VolumePanel(editor: Editor, levels: Memo<MediaLevels>) -> NodeId {
    let output = create_memo(move || levels.with(|levels| levels.output));
    let volume = create_memo(move || output.get().map_or(0.0, |output| output.level));
    let shown = create_memo(move || output.get().map(percent).unwrap_or_default());
    let muted = create_memo(move || output.get().is_some_and(|output| output.muted));
    let listed = create_memo(move || !levels.with(|levels| levels.outputs.is_empty()));
    view! {
        <List spacing=SPACING @test_id={"desktop.volume.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem grow=1.0>
                    <Slider
                        value={Binding::new(volume, move |level: f32| editor.act(move || desktop::media::request(MediaRequest::SetVolume(level)).detach()))}
                        max={MediaLevel::LOUDEST}
                        mark=1.0
                        label="Volume"
                        @test_id={"desktop.volume.level"}
                    />
                </ListItem>
                <ListItem length=PERCENT_WIDTH>
                    <Text>{shown}</Text>
                </ListItem>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <ListItem grow=1.0>
                    <Text>"Mute"</Text>
                </ListItem>
                <Switch
                    on={Binding::new(muted, move |mute: bool| editor.act(move || desktop::media::request(MediaRequest::SetMute(mute)).detach()))}
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
    let selected = create_memo(move || {
        let chosen = levels.with(|levels| levels.default_output.clone())?;
        outputs.with(|outputs| outputs.iter().position(|output| output.id == chosen))
    });
    let choose = move |index: Option<usize>| {
        let chosen = index.and_then(|index| {
            outputs.with_untracked(|outputs| outputs.get(index).map(|output| output.id.clone()))
        });
        if let Some(id) = chosen {
            editor.act(move || desktop::media::request(MediaRequest::ChooseOutput(id)).detach());
        }
    };
    view! {
        <List spacing=LABEL_SPACING>
            <Text style=TextStyle::CAPTION>"Output"</Text>
            <Select
                options={view! {
                    <ForEach keys={keys}>|id: String| {
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
                            <ChoiceOption label />
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
