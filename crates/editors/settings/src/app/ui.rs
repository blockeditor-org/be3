use block_editor_beui::be_block::settings::{ActivationCondition, Settings};
use block_editor_beui::be_block::{
    BlockContent, InputSettingsContent, SettingsContent, UiSettingsContent,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, List, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_beui::{ContentProjection, Editor};
use std::rc::Rc;

use uuid::Uuid;

const PADDING: f32 = 20.0;

#[component]
pub fn SettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<SettingsContent>();
    let loaded = settings.project(|_| true);
    let read_only = editor.read_only();
    let blocked = create_memo(clone!(loaded read_only -> move || !loaded.get() || read_only.get()));
    let open_ui = opener::<UiSettingsContent>(&editor, &settings);
    let open_input = opener::<InputSettingsContent>(&editor, &settings);
    let ui_blocked = blocked.clone();
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
                        disabled={ui_blocked}
                        @test_id={"settings.ui-settings"}
                        on_click={open_ui}
                    />
                    <Button
                        label="Input settings"
                        variant=ButtonVariant::Secondary
                        disabled={blocked}
                        @test_id={"settings.input-settings"}
                        on_click={open_input}
                    />
                </List>
            </List>
        </Frame>
    }
}

fn opener<C: BlockContent + Default>(
    editor: &Editor,
    settings: &Rc<ContentProjection<SettingsContent>>,
) -> impl Fn() + Clone + use<C> {
    let client_id = editor.host().client_id();
    let resolved =
        settings.project(move |settings| settings.root().resolve(C::CONTENT_TYPE, client_id));
    let host = editor.host().clone();
    let creator = editor.clone();
    let settings = settings.clone();
    move || {
        let id = resolved
            .get_untracked()
            .unwrap_or_else(|| create::<C>(&creator, &settings));
        host.open_block(id, C::CONTENT_TYPE);
    }
}

fn create<C: BlockContent + Default>(
    editor: &Editor,
    settings: &ContentProjection<SettingsContent>,
) -> Uuid {
    let block = editor.create_child(&C::default());
    settings.operate(Settings::set_entry(
        C::CONTENT_TYPE,
        ActivationCondition::Fallback,
        block,
    ));
    block
}
