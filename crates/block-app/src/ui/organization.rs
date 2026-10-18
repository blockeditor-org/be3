use beui::NodeId;
use beui::icons::{
    ICON_BUG_REPORT, ICON_CHECK, ICON_CONTACTS, ICON_GPP_MAYBE, ICON_GROUP, ICON_INFO, ICON_KEY,
    ICON_MANAGE_ACCOUNTS, ICON_PERSON_ADD, ICON_POWER_SETTINGS_NEW, ICON_SMART_TOY,
    ICON_SWAP_HORIZ, ICON_SYNC, ICON_VIEW_QUILT,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{Container, MenuItem};
use block_plugin_api::HostPanel;

use super::debug::DebugPanels;
use super::dialogs::Dialogs;
use super::keys::PairingDialog;
use super::{AppViewStore, StatusView, UiCommand, send};
use crate::compositor::PresentingSurface;
use crate::surfaces::MainSurface;

#[component]
pub(super) fn OrganizationScreen(view: AppViewStore) -> NodeId {
    view! {
        <Container>|_| {
            let view = view;
            view! {
                <OrganizationBody view />
            }
        }</Container>
    }
}

#[component]
fn OrganizationBody(view: AppViewStore) -> NodeId {
    let debug = view.debug;
    provide_context(DebugPanels(create_memo(move || debug.get())));
    let presenting = view.presenting;
    let normal = create_memo(move || !presenting.get());
    let status = create_memo(move || view.status.get());
    let menu = view.app_menu;
    view! {
        <List spacing=0.0>
            <Show condition={normal}>
                <MainSurface @sizing=ItemSize::Percent(100.0) />
            </Show>
            <AppMenu status={status} open={menu} />
            <PresentingSurface />
            <Dialogs view={view} />
            <PairingDialog view={view} />
        </List>
    }
}

fn pick(command: UiCommand) {
    send(UiCommand::AppMenu(false));
    send(command);
}

#[component]
fn AppMenu(status: Memo<StatusView>, open: Prop<bool>) -> NodeId {
    let saved = create_memo(move || status.get().changes_saved);
    let unsaved = create_memo(move || !saved.get());
    let saved_label = create_memo(move || match saved.get() {
        true => "All changes saved".to_owned(),
        false => "Submitting changes…".to_owned(),
    });
    let saved_glyph = create_memo(move || match saved.get() {
        true => ICON_CHECK.to_owned(),
        false => ICON_SYNC.to_owned(),
    });
    let organization = create_memo(move || status.get().organization);
    let signed_in_as = create_memo(move || status.get().signed_in_as);
    let panel = |panel: HostPanel| move || pick(UiCommand::ShowPanel(panel));
    let can_close = create_memo(move || status.get().can_close);
    let company = create_memo(move || status.get().company);
    let administrator = create_memo(move || status.get().administrator);
    let escrow_warning = create_memo(move || status.get().escrow_warning.unwrap_or_default());
    let warned = create_memo(move || !escrow_warning.get().is_empty());
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
                <Show condition={warned}>
                    <MenuItem
                        row_test_id={"app.menu.escrow-warning".to_owned()}
                        label="Escrow key not verified"
                        glyph={ICON_GPP_MAYBE.to_owned()}
                        detail={escrow_warning}
                        disabled=true
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.contacts".to_owned()}
                    label="Contacts"
                    glyph={ICON_CONTACTS.to_owned()}
                    separated=true
                    on_click={|| pick(UiCommand::OpenContacts)}
                />
                <Show condition={company}>
                    <MenuItem
                        row_test_id={"app.menu.invite".to_owned()}
                        label="Invite member"
                        glyph={ICON_PERSON_ADD.to_owned()}
                        detail={organization}
                        on_click={|| pick(UiCommand::InviteMember)}
                    />
                </Show>
                <Show condition={administrator}>
                    <MenuItem
                        row_test_id={"app.menu.members".to_owned()}
                        label="Members"
                        glyph={ICON_GROUP.to_owned()}
                        on_click={|| pick(UiCommand::OpenMembers)}
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.agents".to_owned()}
                    label="Agents"
                    glyph={ICON_SMART_TOY.to_owned()}
                    on_click={|| pick(UiCommand::OpenAgents)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-session".to_owned()}
                    label="Sessions and profiles"
                    glyph={ICON_VIEW_QUILT.to_owned()}
                    on_click={|| pick(UiCommand::SwitchSession)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-organization".to_owned()}
                    label="Switch organization"
                    glyph={ICON_SWAP_HORIZ.to_owned()}
                    on_click={|| pick(UiCommand::SwitchOrganization)}
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
    }
}
