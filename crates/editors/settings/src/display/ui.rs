use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    ChangesToast, Displays, Editor, LayeredSettings, LayeredSettingsRow, Power,
};

use super::appearance::AppearanceSection;
use super::power::PowerSection;
use super::screens::ScreenSection;

pub(super) type Settings = LayeredSettings<DisplaySettings>;

#[component]
pub fn DisplaySettingsView(editor: Editor, settings: Settings) -> NodeId {
    let read_only = editor.read_only();
    let merged = settings.effective();
    let offered = editor.host_offers::<Displays>();
    let displays = editor.host_value::<Displays>();
    let ids = create_memo(move || {
        displays.with(|displays| {
            displays
                .iter()
                .map(|display| display.id.clone())
                .collect::<Vec<String>>()
        })
    });
    let none = create_memo(move || displays.with(Vec::is_empty));
    let in_bar = create_memo(move || merged.with(DisplaySettings::rotate_in_bar));
    let rotate_in_bar = DisplaySettings::rotate_in_bar_setting();
    let in_bar_row = settings.row(&rotate_in_bar);
    let show_in_bar = settings.setter(&rotate_in_bar);
    let power = editor.host_offers::<Power>();
    let page = NodeRef::new();
    view! {
        <SettingsPage @node_ref=&page>
            <AppearanceSection settings={settings} read_only={read_only} />
            <Show condition={offered}>
                <SettingsSection title="Screens">
                    <Show condition={none}>
                        <SettingsRow label="No screen is connected to this session." />
                    </Show>
                    <LayeredSettingsRow
                        setting={in_bar_row}
                        label="Rotate button in the bar"
                        description="A button in the desktop's bar that turns the screen."
                        separated={none}
                        compact=true
                        locked={read_only}
                        id={"display.rotate-in-bar".to_owned()}
                    >
                        <Switch
                            on={Binding::new(in_bar, move |on: bool| show_in_bar.call(Some(on)))}
                            label="Rotate button in the bar"
                            disabled={read_only}
                            @test_id={"display.rotate-in-bar"}
                        />
                    </LayeredSettingsRow>
                </SettingsSection>
            </Show>
            <ForEach keys={ids}>|id: String|
                <ScreenSection
                    id={id}
                    settings={settings}
                    displays={displays}
                    read_only={read_only}
                />
            </ForEach>
            <Show condition={power}>
                <PowerSection settings={settings} read_only={read_only} />
            </Show>
            <ChangesToast changes={settings.changes()} anchor={page} />
        </SettingsPage>
    }
}
