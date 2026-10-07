use beui::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_BUG_REPORT, ICON_CHECK, ICON_INFO, ICON_KEY, ICON_MANAGE_ACCOUNTS,
    ICON_PERSON_ADD, ICON_POWER_SETTINGS_NEW, ICON_SWAP_HORIZ, ICON_TERMINAL,
};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component, create_memo,
    provide_context, view,
};
use beui::styled::{ActionRow, Caption, Icon, ModalSheet, Separator, Spinner, use_theme};
use beui::unstyled::Container;
use beui::{Color32, NodeId};
use block_plugin_api::HostPanel;

use super::debug::DebugPanels;
use super::dialogs::Dialogs;
use super::keys::PairingDialog;
use super::{AppViewStore, StatusView, UiCommand, send};
use crate::compositor::PresentingSurface;
use crate::surfaces::MainSurface;

const MENU_PADDING: f32 = 8.0;
const MENU_STOPS: [f32; 2] = [0.6, 0.9];

#[component]
pub(super) fn WorkspaceScreen(view: AppViewStore) -> NodeId {
    view! {
        <Container>
            {move |_| {
                let view = view.clone();
                view! {
                    <WorkspaceBody view />
                }
            }}
        </Container>
    }
}

#[component]
fn WorkspaceBody(view: AppViewStore) -> NodeId {
    let debug = view.debug.clone();
    provide_context(DebugPanels(create_memo(move || debug.get())));
    let presenting = view.presenting.clone();
    let normal = create_memo(clone!(presenting -> move || !presenting.get()));
    let status = create_memo(clone!(view -> move || view.status.get()));
    let menu = view.app_menu.clone();
    view! {
        <List spacing=0.0>
            <Show condition={normal}>
                <MainSurface @sizing=ItemSize::Percent(100.0) />
            </Show>
            <ModalSheet
                open={menu}
                rest={MENU_STOPS[0]}
                stops={MENU_STOPS.to_vec()}
                on_close={|| send(UiCommand::AppMenu(false))}
            >
                <AppMenu status={status} />
            </ModalSheet>
            <Show condition={presenting}>
                <Frame @sizing=ItemSize::Percent(100.0) color=Color32::BLACK>
                    <PresentingSurface />
                </Frame>
            </Show>
            <Dialogs view={view.clone()} />
            <PairingDialog view={view.clone()} />
        </List>
    }
}

fn pick(command: UiCommand) {
    send(UiCommand::AppMenu(false));
    send(command);
}

#[component]
fn AppMenu(status: Memo<StatusView>) -> NodeId {
    let theme = use_theme();
    let saved = create_memo(clone!(status -> move || status.get().changes_saved));
    let unsaved = create_memo(clone!(saved -> move || !saved.get()));
    let saving = unsaved.clone();
    let saved_label = create_memo(clone!(saved -> move || match saved.get() {
        true => "All changes saved".to_owned(),
        false => "Submitting changes…".to_owned(),
    }));
    let workspace = create_memo(clone!(status -> move || status.get().workspace));
    let signed_in_as = create_memo(clone!(status -> move || status.get().signed_in_as));
    let accounts = create_memo(clone!(status -> move || status.get().accounts));
    let account_keys = create_memo(clone!(accounts -> move || {
        accounts
            .get()
            .into_iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
    }));
    let listed = accounts.clone();
    let panel = |panel: HostPanel| move || pick(UiCommand::ShowPanel(panel));
    let runs_programs = create_memo(clone!(status -> move || status.get().runs_programs));
    let can_close = create_memo(clone!(status -> move || status.get().can_close));
    view! {
        <Frame padding_horizontal=MENU_PADDING padding_vertical=MENU_PADDING>
            <List spacing=0.0>
                <Frame
                    padding_horizontal=MENU_PADDING
                    padding_vertical=MENU_PADDING
                    @test_id={"app.menu.status"}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Show condition={saved}>
                            <Icon glyph={ICON_CHECK.to_owned()} color={theme.text_muted.clone()} />
                        </Show>
                        <Show condition={saving}>
                            <Spinner width=24.0 label="Submitting changes" />
                        </Show>
                        <Caption content={saved_label} />
                    </List>
                </Frame>
                <ActionRow
                    @test_id={"app.menu.invite"}
                    label="Invite member"
                    glyph={ICON_PERSON_ADD.to_owned()}
                    detail={workspace}
                    on_click={|| pick(UiCommand::InviteMember)}
                />
                <ActionRow
                    @test_id={"app.menu.switch-workspace"}
                    label="Switch workspace"
                    glyph={ICON_SWAP_HORIZ.to_owned()}
                    on_click={|| pick(UiCommand::SwitchWorkspace)}
                />
                <ActionRow
                    label="New recovery phrase"
                    glyph={ICON_KEY.to_owned()}
                    on_click={|| pick(UiCommand::NewRecoveryPhrase)}
                />
                <ForEach keys={account_keys}>
                    {move |key: String| {
                        let listed = listed.clone();
                        let test_id = format!("app.menu.account.{key}");
                        let label = create_memo(clone!(key -> move || {
                            listed
                                .get()
                                .into_iter()
                                .find(|account| account.key == key)
                                .map(|account| match account.current {
                                    true => format!("{} (current)", account.name),
                                    false => account.name,
                                })
                                .unwrap_or_default()
                        }));
                        view! {
                            <ActionRow
                                @test_id={test_id}
                                label
                                glyph={ICON_ACCOUNT_CIRCLE.to_owned()}
                                on_click={move || pick(UiCommand::SwitchTo(key.clone()))}
                            />
                        }
                    }}
                </ForEach>
                <ActionRow
                    @test_id={"app.menu.accounts"}
                    label="Manage accounts"
                    glyph={ICON_MANAGE_ACCOUNTS.to_owned()}
                    detail={signed_in_as}
                    disabled={unsaved}
                    on_click={|| pick(UiCommand::ManageAccounts)}
                />
                <Show condition={runs_programs}>
                    <ActionRow
                        @test_id={"app.menu.run-program"}
                        label="Run a program"
                        glyph={ICON_TERMINAL.to_owned()}
                        on_click={|| pick(UiCommand::RunProgram(true))}
                    />
                </Show>
                <ActionRow
                    @test_id={"app.menu.about"}
                    label="About"
                    glyph={ICON_INFO.to_owned()}
                    on_click={|| pick(UiCommand::About(true))}
                />
                <Show condition={can_close}>
                    <ActionRow
                        @test_id={"app.menu.close"}
                        label="Close block-app"
                        glyph={ICON_POWER_SETTINGS_NEW.to_owned()}
                        on_click={|| pick(UiCommand::CloseApp)}
                    />
                </Show>
                <Separator />
                <ForEach keys={HostPanel::ALL.to_vec()}>
                    {move |shown: HostPanel| view! {
                        <ActionRow
                            @test_id={format!("app.menu.panel.{shown:?}")}
                            label={shown.title().to_owned()}
                            glyph={panel_icon(shown).to_owned()}
                            on_click={panel(shown)}
                        />
                    }}
                </ForEach>
                <ActionRow
                    label="Inspector"
                    glyph={ICON_BUG_REPORT.to_owned()}
                    on_click={|| pick(UiCommand::OpenInspector)}
                />
            </List>
        </Frame>
    }
}

fn panel_icon(panel: HostPanel) -> &'static str {
    match panel {
        HostPanel::BlockStack => beui::icons::ICON_LAYERS,
        HostPanel::Performance => beui::icons::ICON_SPEED,
        HostPanel::Plugins => beui::icons::ICON_EXTENSION,
        HostPanel::Version => beui::icons::ICON_HISTORY,
    }
}
