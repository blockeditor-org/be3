use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_VOLUME_OFF;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Select, Slider, Switch};
use block_editor_beui::beui::unstyled::{ChoiceOption, PopoverHandle};
use block_editor_beui::{Editor, MediaLevel, MediaLevels, MediaRequest};

use super::media::volume_glyph;
use super::popup::BarPopup;
use block_editor_beui::beui::styled::{Text, TextStyle};

const VOLUME_WIDTH: f32 = 280.0;
const SPACING: f32 = 12.0;
const LABEL_SPACING: f32 = 4.0;
const PERCENT_WIDTH: f32 = 44.0;

fn percent(level: MediaLevel) -> String {
    format!("{}%", (level.level * 100.0).round())
}

#[component]
pub(crate) fn VolumeButton(editor: Editor, levels: Memo<MediaLevels>) -> NodeId {
    let output = create_memo(move || levels.with(|levels| levels.output));
    let glyph = create_memo(move || {
        output
            .get()
            .map_or(ICON_VOLUME_OFF, volume_glyph)
            .to_owned()
    });
    let label = create_memo(move || output.get().map(percent).unwrap_or_default());
    view! {
        <BarPopup label glyph width=VOLUME_WIDTH @test_id={"desktop.volume"}>
            {move |_: PopoverHandle| view! {
                <VolumePanel editor={editor} levels={levels} />
            }}
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
    let (setting, muting) = (editor, editor);
    view! {
        <List spacing=SPACING @test_id={"desktop.volume.panel"}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Slider
                    @sizing=ItemSize::Percent(100.0)
                    value={volume}
                    label="Volume"
                    @test_id={"desktop.volume.level"}
                    on_change={move |level: f32| setting.act(MediaRequest::SetVolume(level))}
                />
                <Text @sizing=ItemSize::Fixed(PERCENT_WIDTH) string={shown} />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text @sizing=ItemSize::Percent(100.0) string="Mute" />
                <Switch
                    on={muted}
                    label="Mute"
                    @test_id={"desktop.volume.mute"}
                    on_change={move |mute: bool| muting.act(MediaRequest::SetMute(mute))}
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
    let named = outputs;
    let choose = move |index: Option<usize>| {
        let chosen = index.and_then(|index| {
            outputs.with_untracked(|outputs| outputs.get(index).map(|output| output.id.clone()))
        });
        if let Some(id) = chosen {
            editor.act(MediaRequest::ChooseOutput(id));
        }
    };
    view! {
        <List spacing=LABEL_SPACING>
            <Text style=TextStyle::CAPTION string="Output" />
            <Select
                options={view! {
                    <ForEach keys={keys}>
                        {move |id: String| {
                            let label = create_memo(move || {
                                named.with(|outputs| {
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
                        }}
                    </ForEach>
                }}
                selected={selected}
                label="Output"
                @test_id={"desktop.volume.output"}
                on_change={choose}
            />
        </List>
    }
}
