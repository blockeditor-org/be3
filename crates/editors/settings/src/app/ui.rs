use block::{Block, BlockParent};
use block_client::blocks::ui_settings::UiSettings;
use block_editor_plugin::be_block::settings::{ActivationCondition, Settings};
use block_editor_plugin::be_block::{SettingsContent, UiSettingsContent};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, List, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::{ContentProjection, Editor};
use uuid::Uuid;

const PADDING: f32 = 20.0;

#[component]
pub fn SettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<SettingsContent>();
    let client_id = editor.host().client_id();
    let loaded = settings.project(|_| true);
    let ui_settings =
        settings.project(move |settings| settings.root().resolve(UiSettings::TYPE_ID, client_id));
    let read_only = editor.read_only();
    let blocked = create_memo(clone!(loaded read_only -> move || !loaded.get() || read_only.get()));
    let host = editor.host().clone();
    let creator = editor.clone();
    let open = clone!(settings ui_settings -> move || {
        let Some(id) = ui_settings
            .get_untracked()
            .or_else(|| create_ui_settings(&creator, &settings))
        else {
            return;
        };
        host.open_block(id, UiSettings::TYPE_ID);
    });
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=10.0>
                <Heading content="Settings" />
                <Caption content="Settings this workspace resolves for every editor." />
                <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                    <Button
                        label="UI settings"
                        variant=ButtonVariant::Primary
                        disabled={blocked}
                        @test_id={"settings.ui-settings"}
                        on_click={open}
                    />
                </List>
            </List>
        </Frame>
    }
}

fn create_ui_settings(
    editor: &Editor,
    settings: &ContentProjection<SettingsContent>,
) -> Option<Uuid> {
    let block = editor.create_with_content::<UiSettings, _>(&UiSettingsContent::default());
    settings.operate(Settings::set_entry(
        UiSettings::TYPE_ID,
        ActivationCondition::Fallback,
        block.id(),
    ));
    block.set_parent(BlockParent::Uuid(editor.block_id()));
    Some(block.id())
}
