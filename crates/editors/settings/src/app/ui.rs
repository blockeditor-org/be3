use std::sync::Arc;

use block::{Block, BlockParent};
use block_client::BlockClient;
use block_client::block_ref::BlockRef;
use block_client::blocks::settings::{ActivationCondition, Settings, SettingsOperation};
use block_client::blocks::ui_settings::UiSettings;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    CenteredRow, Column, Frame, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::{BlockProjection, Editor};
use uuid::Uuid;

const PADDING: f32 = 20.0;

#[component]
pub fn SettingsView(editor: Editor) -> NodeId {
    let settings = editor.block::<Settings>();
    let client_id = editor.host().client_id();
    let loaded = settings.project(|_| true);
    let ui_settings = settings.project(move |settings| {
        settings
            .resolve(UiSettings::TYPE_ID, client_id)
            .and_then(|reference| reference.as_direct())
    });
    let read_only = editor.read_only();
    let blocked = create_memo(clone!(loaded read_only -> move || !loaded.get() || read_only.get()));
    let host = editor.host().clone();
    let client = editor.client().clone();
    let open = clone!(settings ui_settings -> move || {
        let Some(id) = ui_settings
            .get_untracked()
            .or_else(|| create_ui_settings(&client, &settings))
        else {
            return;
        };
        host.open_block(id, UiSettings::TYPE_ID);
    });
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <Column spacing=10.0>
                <Heading content="Settings" />
                <Caption content="Settings this workspace resolves for every editor." />
                <CenteredRow spacing=10.0>
                    <Button
                        label="UI settings"
                        variant=ButtonVariant::Primary
                        disabled={blocked}
                        @test_id={"settings.ui-settings"}
                        on_click={open}
                    />
                </CenteredRow>
            </Column>
        </Frame>
    }
}

fn create_ui_settings(
    client: &Arc<BlockClient>,
    settings: &BlockProjection<Settings>,
) -> Option<Uuid> {
    let block = client.create_block(UiSettings::new());
    settings.operate(SettingsOperation::SetEntry {
        block_type: UiSettings::TYPE_ID,
        activation: ActivationCondition::Fallback,
        block: BlockRef::Direct(block.id()),
    });
    block.set_parent(BlockParent::Uuid(settings.id()));
    Some(block.id())
}
