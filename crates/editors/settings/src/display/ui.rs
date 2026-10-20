use block_editor_beui::be_block::{DisplaySettings, DisplaySettingsContent, Layered};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{ContentProjection, Displays, Editor, Power};

use super::appearance::AppearanceSection;
use super::power::PowerSection;
use super::screens::ScreenSection;

pub(super) type Settings = ContentProjection<DisplaySettingsContent>;

#[component]
pub fn DisplaySettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<DisplaySettingsContent>();
    let read_only = editor.read_only();
    let root = settings.project(|content| content.root());
    let inherited = editor.inherited_settings::<DisplaySettings>();
    let merged = create_memo(move || root.get().over(&inherited.get()));
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
    let in_bar_set = create_memo(move || root.with(|root| root.rotate_in_bar.is_some()));
    let power = editor.host_offers::<Power>();
    view! {
        <SettingsPage>
            <AppearanceSection
                settings={settings}
                root={root}
                inherited={inherited}
                read_only={read_only}
            />
            <Show condition={offered}>
                <SettingsSection title="Screens">
                    <Show condition={none}>
                        <SettingsRow label="No screen is connected to this session." />
                    </Show>
                    <SettingsRow
                        label="Rotate button in the bar"
                        description="A button in the desktop's bar that turns the screen."
                        separated={none}
                        compact=true
                        modified={in_bar_set}
                        locked={read_only}
                        reset_test_id="display.rotate-in-bar.reset"
                        on_reset={move || settings.operate(DisplaySettings::set_rotate_in_bar(None))}
                    >
                        <Switch
                            on={in_bar}
                            label="Rotate button in the bar"
                            disabled={read_only}
                            @test_id={"display.rotate-in-bar"}
                            on_change={move |on: bool| settings.operate(DisplaySettings::set_rotate_in_bar(Some(on)))}
                        />
                    </SettingsRow>
                </SettingsSection>
            </Show>
            <ForEach keys={ids}>|id: String|
                <ScreenSection
                    id={id}
                    settings={settings}
                    root={root}
                    merged={merged}
                    displays={displays}
                    read_only={read_only}
                />
            </ForEach>
            <Show condition={power}>
                <PowerSection
                    settings={settings}
                    root={root}
                    merged={merged}
                    read_only={read_only}
                />
            </Show>
        </SettingsPage>
    }
}
