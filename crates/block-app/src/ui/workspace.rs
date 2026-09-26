use beui::icons::ICON_CHECK;
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_memo, view,
};
use beui::styled::{Caption, Icon, MenuButton, Separator, Spinner, use_theme};
use beui::unstyled::MenuItem;
use beui::{Color32, NodeId};

use super::debug::{DebugCommand, DebugWindow};
use super::dialogs::Dialogs;
use super::picker::PickerDialogs;
use super::share::ShareWindow;
use super::tools::WorkspaceDock;
use super::{AppViewStore, StatusView, UiCommand, send};
use crate::surfaces::{HostSurface, SurfaceId};

const STATUS_PADDING_HORIZONTAL: f32 = 12.0;
const STATUS_PADDING_VERTICAL: f32 = 4.0;

#[component]
pub(super) fn WorkspaceScreen(view: AppViewStore) -> NodeId {
    let presenting = view.presenting.clone();
    let normal = create_memo(clone!(presenting -> move || !presenting.get()));
    let status = create_memo(clone!(view -> move || view.status.get()));
    let docked = view.clone();
    view! {
        <List spacing=0.0>
            <Show condition={normal.clone()}>
                <WorkspaceDock @sizing=ItemSize::Percent(100.0) view={docked} />
            </Show>
            <Show condition={normal.clone()}>
                <Separator />
            </Show>
            <Show condition={normal}>
                <StatusBar status />
            </Show>
            <Show condition={presenting}>
                <Frame @sizing=ItemSize::Percent(100.0) color=Color32::BLACK>
                    <HostSurface id=SurfaceId::Presenting />
                </Frame>
            </Show>
            <Dialogs view={view.clone()} />
            <ShareWindow view={view.clone()} />
            <PickerDialogs view={view.clone()} />
        </List>
    }
}

const MORE_SETTINGS: usize = 0;
const MORE_CLIENT: usize = 1;
const MORE_PERFORMANCE: usize = 2;
const MORE_PLUGINS: usize = 3;
const MORE_VERSION: usize = 4;
const MORE_INSPECTOR: usize = 5;
const MORE_WORKSPACE: usize = 6;
const MORE_ACCOUNTS: usize = 7;
const MORE_ABOUT: usize = 8;

#[component]
fn StatusBar(status: Memo<StatusView>) -> NodeId {
    let theme = use_theme();
    let saved = create_memo(clone!(status -> move || status.get().changes_saved));
    let unsaved = create_memo(clone!(saved -> move || !saved.get()));
    let saved_label = create_memo(clone!(saved -> move || match saved.get() {
        true => "All changes saved".to_owned(),
        false => "Submitting changes…".to_owned(),
    }));
    let frame = create_memo(clone!(status -> move || status.get().frame));
    let workspace = create_memo(clone!(status -> move || {
        format!("Workspace: {}", status.get().workspace)
    }));
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
    let chosen = accounts;
    view! {
        <Frame
            color={theme.surface.clone()}
            padding_horizontal=STATUS_PADDING_HORIZONTAL
            padding_vertical=STATUS_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Show condition={saved}>
                    <Icon glyph={ICON_CHECK.to_owned()} text_size=12.0 />
                </Show>
                <Show condition={unsaved.clone()}>
                    <Spinner width=24.0 label="Submitting changes" />
                </Show>
                <Caption content={saved_label} />
                <Separator direction=Direction::Vertical length={Some(12.0)} />
                <Caption content={frame} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <MenuButton
                    label="More"
                    on_select={move |path: Vec<usize>| more(&path, &chosen.get_untracked())}
                    items={view! {
                        <MenuItem label="Settings" />
                        <MenuItem label="Block stack" />
                        <MenuItem label="Performance" />
                        <MenuItem label="Plugins" />
                        <MenuItem label="Version" />
                        <MenuItem label="Inspector" />
                        <MenuItem label={workspace}>
                            <MenuItem label="Invite member" />
                            <MenuItem label="Switch workspace" />
                        </MenuItem>
                        <MenuItem label={signed_in_as}>
                            <ForEach keys={account_keys}>
                                {move |key: String| {
                                    let listed = listed.clone();
                                    let label = create_memo(move || {
                                        listed
                                            .get()
                                            .into_iter()
                                            .find(|account| account.key == key)
                                            .map(|account| match account.current {
                                                true => format!("{} (current)", account.name),
                                                false => account.name,
                                            })
                                            .unwrap_or_default()
                                    });
                                    view! {
                                        <MenuItem label={label} />
                                    }
                                }}
                            </ForEach>
                            <MenuItem label="Manage accounts" disabled={unsaved.clone()} />
                        </MenuItem>
                        <MenuItem label="About" />
                    }}
                />
            </List>
        </Frame>
    }
}

fn more(path: &[usize], accounts: &[super::AccountRow]) {
    let open = |window| send(UiCommand::Debug(DebugCommand::Open(window)));
    match path {
        [MORE_SETTINGS] => send(UiCommand::OpenSettings),
        [MORE_CLIENT] => open(DebugWindow::Client),
        [MORE_PERFORMANCE] => open(DebugWindow::Performance),
        [MORE_PLUGINS] => open(DebugWindow::Plugins),
        [MORE_VERSION] => open(DebugWindow::Version),
        [MORE_INSPECTOR] => send(UiCommand::OpenInspector),
        [MORE_WORKSPACE, 0] => send(UiCommand::InviteMember),
        [MORE_WORKSPACE, 1] => send(UiCommand::SwitchWorkspace),
        [MORE_ACCOUNTS, index] => match accounts.get(*index) {
            Some(account) => send(UiCommand::SwitchTo(account.key.clone())),
            None => send(UiCommand::ManageAccounts),
        },
        [MORE_ABOUT] => send(UiCommand::About(true)),
        _ => {}
    }
}
