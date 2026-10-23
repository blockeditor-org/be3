use beui::icons::{
    ICON_ARROW_LEFT_RIGHT, ICON_BOT, ICON_BUG, ICON_CHECK, ICON_CONTACT, ICON_INFO, ICON_KEY,
    ICON_LAYOUT_PANEL_LEFT, ICON_POWER, ICON_REFRESH_CCW, ICON_SHIELD_ALERT, ICON_USER_COG,
    ICON_USER_PLUS, ICON_USERS,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{Container, MenuItem};
use beui::{NodeId, OverlayAnchor, Pos2};
use block_plugin_api::HostPanel;

use super::debug::DebugPanels;
use super::dialogs::Dialogs;
use super::keys::PairingDialog;
use super::{AppMenuView, AppViewStore, StatusView, UiCommand, send};
use crate::compositor::PresentingSurface;
use crate::surfaces::MainSurface;

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
    let menu = view.app_menu;
    view! {
        <List spacing=0.0>
            <Show condition={normal}>
                <MainSurface @sizing=ItemSize::Percent(100.0) />
            </Show>
            <AppMenu status={status} menu={menu} />
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
fn AppMenu(status: Memo<StatusView>, menu: Prop<Option<AppMenuView>>) -> NodeId {
    let menu = create_memo(move || menu.get());
    let open = create_memo(move || menu.with(Option::is_some));
    let placed = create_memo(move || {
        let opened = menu.get()?;
        let pointer = untrack(last_pointer);
        let anchor = match opened.at {
            Some(rect) => OverlayAnchor::Rect(rect),
            None => OverlayAnchor::Point(pointer.map_or(Pos2::ZERO, |pointer| pointer.pos)),
        };
        Some((anchor, pointer.is_some_and(|pointer| pointer.touch)))
    });
    let anchor = create_memo(move || {
        placed
            .get()
            .map_or(OverlayAnchor::Point(Pos2::ZERO), |(anchor, _)| anchor)
    });
    let sheet = create_memo(move || placed.get().is_some_and(|(_, touch)| touch));
    let saved = create_memo(move || status.get().changes_saved);
    let unsaved = create_memo(move || !saved.get());
    let saved_label = create_memo(move || match saved.get() {
        true => "All changes saved".to_owned(),
        false => "Submitting changes…".to_owned(),
    });
    let saved_glyph = create_memo(move || match saved.get() {
        true => ICON_CHECK.to_owned(),
        false => ICON_REFRESH_CCW.to_owned(),
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
        <MenuPopup
            anchor
            open
            sheet
            on_dismiss={|| send(UiCommand::AppMenu(false))}
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
                        glyph={ICON_SHIELD_ALERT.to_owned()}
                        detail={escrow_warning}
                        disabled=true
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.contacts".to_owned()}
                    label="Contacts"
                    glyph={ICON_CONTACT.to_owned()}
                    separated=true
                    on_click={|| pick(UiCommand::OpenContacts)}
                />
                <Show condition={company}>
                    <MenuItem
                        row_test_id={"app.menu.invite".to_owned()}
                        label="Invite member"
                        glyph={ICON_USER_PLUS.to_owned()}
                        detail={organization}
                        on_click={|| pick(UiCommand::InviteMember)}
                    />
                </Show>
                <Show condition={administrator}>
                    <MenuItem
                        row_test_id={"app.menu.members".to_owned()}
                        label="Members"
                        glyph={ICON_USERS.to_owned()}
                        on_click={|| pick(UiCommand::OpenMembers)}
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.agents".to_owned()}
                    label="Agents"
                    glyph={ICON_BOT.to_owned()}
                    on_click={|| pick(UiCommand::OpenAgents)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-session".to_owned()}
                    label="Sessions and profiles"
                    glyph={ICON_LAYOUT_PANEL_LEFT.to_owned()}
                    on_click={|| pick(UiCommand::SwitchSession)}
                />
                <MenuItem
                    row_test_id={"app.menu.switch-organization".to_owned()}
                    label="Switch organization"
                    glyph={ICON_ARROW_LEFT_RIGHT.to_owned()}
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
                    glyph={ICON_USER_COG.to_owned()}
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
                        glyph={ICON_POWER.to_owned()}
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
                    glyph={ICON_BUG.to_owned()}
                    on_click={|| pick(UiCommand::OpenInspector)}
                />
            }}
        />
    }
}

fn panel_icon(panel: HostPanel) -> &'static str {
    match panel {
        HostPanel::BlockStack => beui::icons::ICON_LAYERS,
        HostPanel::Performance => beui::icons::ICON_GAUGE,
        HostPanel::Plugins => beui::icons::ICON_PUZZLE,
    }
}
