use beui::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_ADD, ICON_BUG_REPORT, ICON_CHECK, ICON_DEVICES, ICON_INFO, ICON_KEY,
    ICON_MANAGE_ACCOUNTS, ICON_PERSON_ADD, ICON_SWAP_HORIZ, ICON_SYNC, ICON_TERMINAL,
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

fn profile_label(profile: &super::ProfileRow) -> String {
    match profile.current {
        true => format!("{} (current)", profile.name),
        false => profile.name.clone(),
    }
}

fn profile_ids(status: &Memo<StatusView>) -> Memo<Vec<uuid::Uuid>> {
    create_memo(clone!(status -> move || {
        status
            .get()
            .profiles
            .into_iter()
            .map(|profile| profile.id)
            .collect::<Vec<_>>()
    }))
}

fn profile_name(status: &Memo<StatusView>, id: uuid::Uuid) -> Memo<String> {
    create_memo(clone!(status -> move || {
        status
            .get()
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(profile_label)
            .unwrap_or_default()
    }))
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
    let accounts = create_memo(clone!(status -> move || status.get().accounts));
    let account_keys = create_memo(clone!(accounts -> move || {
        accounts
            .get()
            .into_iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
    }));
    let profile_keys = profile_ids(&status);
    let profile_names = status.clone();
    let listed = accounts.clone();
    let panel = |panel: HostPanel| move || pick(UiCommand::ShowPanel(panel));
    let runs_programs = create_memo(clone!(status -> move || status.get().runs_programs));
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
                <ForEach keys={profile_keys}>
                    {move |id: uuid::Uuid| {
                        let label = profile_name(&profile_names, id);
                        view! {
                            <MenuItem
                                row_test_id={format!("app.menu.profile.{id}")}
                                label
                                glyph={ICON_DEVICES.to_owned()}
                                on_click={move || pick(UiCommand::SwitchProfile(id))}
                            />
                        }
                    }}
                </ForEach>
                <MenuItem
                    row_test_id={"app.menu.new-profile".to_owned()}
                    label="New profile"
                    glyph={ICON_ADD.to_owned()}
                    on_click={|| pick(UiCommand::NewProfile)}
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
                            <MenuItem
                                row_test_id={test_id}
                                label
                                glyph={ICON_ACCOUNT_CIRCLE.to_owned()}
                                on_click={move || pick(UiCommand::SwitchTo(key.clone()))}
                            />
                        }
                    }}
                </ForEach>
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
                        row_test_id={"app.menu.run-program".to_owned()}
                        label="Run a program"
                        glyph={ICON_TERMINAL.to_owned()}
                        on_click={|| pick(UiCommand::RunProgram(true))}
                    />
                </Show>
                <MenuItem
                    row_test_id={"app.menu.about".to_owned()}
                    label="About"
                    glyph={ICON_INFO.to_owned()}
                    on_click={|| pick(UiCommand::About(true))}
                />
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
