use beui::icons::{
    ICON_APPS, ICON_BUG_REPORT, ICON_CHECK, ICON_INFO, ICON_KEY, ICON_MANAGE_ACCOUNTS,
    ICON_PERSON_ADD, ICON_POWER_SETTINGS_NEW, ICON_SWAP_HORIZ, ICON_SYNC,
};
use beui::reactive::{
    ForEach, Frame, ItemSize, List, Memo, Prop, Show, clone, component, create_memo,
    provide_context, view,
};
use beui::styled::ContextMenu;
use beui::unstyled::{Container, MenuItem};
use beui::{Color32, NodeId};
use block_plugin_api::HostPanel;

use super::debug::DebugPanels;
use super::dialogs::Dialogs;
use super::keys::PairingDialog;
use super::{AppViewStore, StatusView, UiCommand, send};
use crate::compositor::PresentingSurface;
use crate::surfaces::MainSurface;

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
            <AppMenu status={status} open={menu} />
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
fn AppMenu(status: Memo<StatusView>, open: Prop<bool>) -> NodeId {
    let saved = create_memo(clone!(status -> move || status.get().changes_saved));
    let unsaved = create_memo(clone!(saved -> move || !saved.get()));
    let saved_label = create_memo(clone!(saved -> move || match saved.get() {
        true => "All changes saved".to_owned(),
        false => "Submitting changes…".to_owned(),
    }));
    let saved_glyph = create_memo(clone!(saved -> move || match saved.get() {
        true => ICON_CHECK.to_owned(),
        false => ICON_SYNC.to_owned(),
    }));
    let workspace = create_memo(clone!(status -> move || status.get().workspace));
    let signed_in_as = create_memo(clone!(status -> move || status.get().signed_in_as));
    let panel = |panel: HostPanel| move || pick(UiCommand::ShowPanel(panel));
    let runs_programs = create_memo(clone!(status -> move || status.get().runs_programs));
    let can_close = create_memo(clone!(status -> move || status.get().can_close));
    view! {
        <ContextMenu
            disabled=true
            open_at_pointer={open}
            on_close={|| send(UiCommand::AppMenu(false))}
            items={view! {
                <MenuItem
                    row_test_id={"app.menu.status".to_owned()}
                    label={saved_label}
                    glyph={saved_glyph}
                    disabled=true
                />
                <MenuItem
                    row_test_id={"app.menu.invite".to_owned()}
                    label="Invite member"
                    glyph={ICON_PERSON_ADD.to_owned()}
                    detail={workspace}
                    separated=true
                    on_click={|| pick(UiCommand::InviteMember)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-workspace".to_owned()}
                    label="Switch workspace"
                    glyph={ICON_SWAP_HORIZ.to_owned()}
                    on_click={|| pick(UiCommand::SwitchWorkspace)}
                />
                <MenuItem
                    label="New recovery phrase"
                    glyph={ICON_KEY.to_owned()}
                    on_click={|| pick(UiCommand::NewRecoveryPhrase)}
                />
                <MenuItem
                    row_test_id={"app.menu.accounts".to_owned()}
                    label="Manage accounts"
                    glyph={ICON_MANAGE_ACCOUNTS.to_owned()}
                    detail={signed_in_as}
                    disabled={unsaved}
                    on_click={|| pick(UiCommand::ManageAccounts)}
                />
                <Show condition={runs_programs}>
                    <MenuItem
                        row_test_id={"app.menu.programs".to_owned()}
                        label="Programs"
                        glyph={ICON_APPS.to_owned()}
                        on_click={|| pick(UiCommand::Launcher(true))}
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.about".to_owned()}
                    label="About"
                    glyph={ICON_INFO.to_owned()}
                    on_click={|| pick(UiCommand::About(true))}
                />
                <Show condition={can_close}>
                    <MenuItem
                        row_test_id={"app.menu.close".to_owned()}
                        label="Close block-app"
                        glyph={ICON_POWER_SETTINGS_NEW.to_owned()}
                        on_click={|| pick(UiCommand::CloseApp)}
                    />
                </Show>
                <ForEach keys={HostPanel::ALL.to_vec()}>
                    {move |shown: HostPanel| view! {
                        <MenuItem
                            row_test_id={format!("app.menu.panel.{shown:?}")}
                            label={shown.title().to_owned()}
                            glyph={panel_icon(shown).to_owned()}
                            separated={shown == HostPanel::ALL[0]}
                            on_click={panel(shown)}
                        />
                    }}
                </ForEach>
                <MenuItem
                    label="Inspector"
                    glyph={ICON_BUG_REPORT.to_owned()}
                    on_click={|| pick(UiCommand::OpenInspector)}
                />
            }}
        >
            <Frame />
        </ContextMenu>
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
