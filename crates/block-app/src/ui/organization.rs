use beui::NodeId;
use beui::icons::Glyph;
use beui::icons::{
    ICON_ARCHIVE_RESTORE, ICON_ARROW_LEFT_RIGHT, ICON_BOT, ICON_BUG, ICON_CLOUD_ALERT,
    ICON_CLOUD_CHECK, ICON_CLOUD_OFF, ICON_CLOUD_SYNC, ICON_CONTACT, ICON_INFO, ICON_KEY,
    ICON_LAYOUT_PANEL_LEFT, ICON_POWER, ICON_SHIELD_ALERT, ICON_USER_COG, ICON_USER_PLUS,
    ICON_USERS,
};
use beui::reactive::*;
use beui::styled::*;
use block_plugin_api::HostPanel;

use super::debug::DebugPanels;
use super::dialogs::Dialogs;
use super::keys::PairingDialog;
use super::{AppViewStore, StatusView, SyncState, UiCommand, send};
use crate::compositor::{ConflictSurface, PresentingSurface};
use crate::surfaces::MainSurface;
use beui::ItemSize;

#[component]
pub(super) fn OrganizationScreen(view: AppViewStore) -> NodeId {
    view! {
        <Container>|_|
            <OrganizationBody view />
        </Container>
    }
}

#[component]
fn OrganizationBody(view: AppViewStore) -> NodeId {
    let debug = view.debug;
    provide_context(DebugPanels(create_memo(move || debug.get())));
    let presenting = view.presenting;
    let normal = create_memo(move || !presenting.get());
    let status = create_memo(move || view.status.get());
    provide_context(AppStatus(status));
    view! {
        <List spacing=Space::None>
            <Show condition={normal}>
                <MainSurface @sizing=ItemSize::Percent(100.0) />
            </Show>
            <PresentingSurface />
            <ConflictSurface />
            <Dialogs view={view} />
            <PairingDialog view={view} />
        </List>
    }
}

fn sync_label(sync: &SyncState) -> &'static str {
    match sync {
        SyncState::Saved => "All changes saved",
        SyncState::Connecting => "Connecting…",
        SyncState::Syncing => "Syncing changes…",
        SyncState::Offline { .. } => "Offline",
        SyncState::Error(_) => "Changes could not sync",
    }
}

fn sync_detail(sync: &SyncState) -> String {
    match sync {
        SyncState::Saved | SyncState::Connecting | SyncState::Syncing => String::new(),
        SyncState::Offline { waiting: 0 } => "Nothing waiting to sync".to_owned(),
        SyncState::Offline { waiting: 1 } => "1 change kept on this device to sync".to_owned(),
        SyncState::Offline { waiting } => format!("{waiting} changes kept on this device to sync"),
        SyncState::Error(error) => error.clone(),
    }
}

fn sync_glyph(sync: &SyncState) -> Glyph {
    match sync {
        SyncState::Saved => ICON_CLOUD_CHECK,
        SyncState::Connecting | SyncState::Syncing => ICON_CLOUD_SYNC,
        SyncState::Offline { .. } => ICON_CLOUD_OFF,
        SyncState::Error(_) => ICON_CLOUD_ALERT,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AppStatus(pub(crate) Memo<StatusView>);

#[component]
pub(crate) fn AppMenuPanel() -> NodeId {
    let report = use_context::<crate::compositor::PanelReport>();
    let status = use_context::<AppStatus>().map_or_else(
        || create_memo(StatusView::default),
        |AppStatus(status)| status,
    );
    let pick = move |command: UiCommand| {
        if let Some(report) = report {
            report.done.set(true);
        }
        send(command);
    };
    view! {
        <AppMenu status pick={Callback::new(pick)} report />
    }
}

#[component]
fn AppMenu(
    status: Memo<StatusView>,
    pick: Callback<UiCommand>,
    report: Option<crate::compositor::PanelReport>,
) -> NodeId {
    let pick = move |command: UiCommand| pick.call(command);
    let sync = create_memo(move || status.get().sync);
    let sync_label = create_memo(move || sync_label(&sync.get()).to_owned());
    let sync_detail = create_memo(move || sync_detail(&sync.get()));
    let sync_glyph = create_memo(move || sync_glyph(&sync.get()));
    let unsaved = create_memo(move || status.get().unsaved);
    let unsent = create_memo(move || status.get().unsent);
    let has_unsent = create_memo(move || unsent.get() > 0);
    let unsent_detail = create_memo(move || match unsent.get() {
        1 => "1 block".to_owned(),
        count => format!("{count} blocks"),
    });
    let organization = create_memo(move || status.get().organization);
    let signed_in_as = create_memo(move || status.get().signed_in_as);
    let panel = move |panel: HostPanel| move || pick(UiCommand::ShowPanel(panel));
    let can_close = create_memo(move || status.get().can_close);
    let company = create_memo(move || status.get().company);
    let administrator = create_memo(move || status.get().administrator);
    let escrow_warning = create_memo(move || status.get().escrow_warning.unwrap_or_default());
    let warned = create_memo(move || !escrow_warning.get().is_empty());
    let rows = view! {
        <MenuRows
            active=true
            items={view! {
                <MenuItem
                    row_test_id={"app.menu.status".to_owned()}
                    label={sync_label}
                    glyph={sync_glyph}
                    detail={sync_detail}
                    disabled=true
                />
                <Show condition={has_unsent}>
                    <MenuItem
                        row_test_id={"app.menu.unsent".to_owned()}
                        label="Unsent edits"
                        glyph=ICON_ARCHIVE_RESTORE
                        detail={unsent_detail}
                        on_click={move || pick(UiCommand::Unsent(true))}
                    />
                </Show>
                <Show condition={warned}>
                    <MenuItem
                        row_test_id={"app.menu.escrow-warning".to_owned()}
                        label="Escrow key not verified"
                        glyph=ICON_SHIELD_ALERT
                        detail={escrow_warning}
                        disabled=true
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.contacts".to_owned()}
                    label="Contacts"
                    glyph=ICON_CONTACT
                    separated=true
                    on_click={move || pick(UiCommand::OpenContacts)}
                />
                <Show condition={company}>
                    <MenuItem
                        row_test_id={"app.menu.invite".to_owned()}
                        label="Invite member"
                        glyph=ICON_USER_PLUS
                        detail={organization}
                        on_click={move || pick(UiCommand::InviteMember)}
                    />
                </Show>
                <Show condition={administrator}>
                    <MenuItem
                        row_test_id={"app.menu.members".to_owned()}
                        label="Members"
                        glyph=ICON_USERS
                        on_click={move || pick(UiCommand::OpenMembers)}
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.agents".to_owned()}
                    label="Agents"
                    glyph=ICON_BOT
                    on_click={move || pick(UiCommand::OpenAgents)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-session".to_owned()}
                    label="Sessions and profiles"
                    glyph=ICON_LAYOUT_PANEL_LEFT
                    on_click={move || pick(UiCommand::SwitchSession)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-organization".to_owned()}
                    label="Switch organization"
                    glyph=ICON_ARROW_LEFT_RIGHT
                    on_click={move || pick(UiCommand::SwitchOrganization)}
                />
                <MenuItem
                    label="New recovery phrase"
                    glyph=ICON_KEY
                    on_click={move || pick(UiCommand::NewRecoveryPhrase)}
                />
                <MenuItem
                    row_test_id={"app.menu.accounts".to_owned()}
                    label="Manage accounts"
                    glyph=ICON_USER_COG
                    detail={signed_in_as}
                    disabled={unsaved}
                    on_click={move || pick(UiCommand::ManageAccounts)}
                />
                <MenuItem
                    row_test_id={"app.menu.about".to_owned()}
                    label="About"
                    glyph=ICON_INFO
                    on_click={move || pick(UiCommand::About(true))}
                />
                <Show condition={can_close}>
                    <MenuItem
                        row_test_id={"app.menu.close".to_owned()}
                        label="Close block-app"
                        glyph=ICON_POWER
                        on_click={move || pick(UiCommand::CloseApp)}
                    />
                </Show>
                <ForEach keys={HostPanel::ALL.to_vec()}>|shown: HostPanel|
                    <MenuItem
                        row_test_id={format!("app.menu.panel.{shown:?}")}
                        label={shown.title().to_owned()}
                        glyph={panel_icon(shown).to_owned()}
                        separated={shown == HostPanel::ALL[0]}
                        on_click={panel(shown)}
                    />
                </ForEach>
                <MenuItem
                    label="Inspector"
                    glyph=ICON_BUG
                    on_click={move || pick(UiCommand::OpenInspector)}
                />
            }}
        />
    };
    if let Some(report) = report {
        let held = NodeRef::new();
        held.fill(rows);
        on_key(
            beui::KeyAt::Node(held),
            beui::KeyPhase::Bubble,
            move |event| {
                let escaped = event
                    .press()
                    .is_some_and(|press| press.pressed && press.key == beui::Key::Escape);
                if escaped {
                    report.done.set(true);
                }
                escaped
            },
        );
        let measured = measure(rows);
        create_effect(move || {
            if let Some(measured) = measured.get() {
                report.size.set(Some(measured.rect.size()));
            }
        });
    }
    view! {
        <Scroll>
            <Frame align={beui::Align2::START}>{rows}</Frame>
        </Scroll>
    }
}

fn panel_icon(panel: HostPanel) -> Glyph {
    match panel {
        HostPanel::BlockStack => beui::icons::ICON_LAYERS,
        HostPanel::Performance => beui::icons::ICON_GAUGE,
        HostPanel::Plugins => beui::icons::ICON_PUZZLE,
        HostPanel::AppMenu => beui::icons::ICON_MENU,
    }
}

#[cfg(test)]
mod tests;
