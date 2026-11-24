//! The root's menu, which opens what be-host's organization menu does.

use super::*;
use beui::icons::{
    ICON_ARCHIVE_RESTORE, ICON_ARROW_LEFT_RIGHT, ICON_BOT, ICON_CLOUD_ALERT, ICON_CLOUD_CHECK,
    ICON_CLOUD_OFF, ICON_CLOUD_SYNC, ICON_CONTACT, ICON_GAUGE, ICON_KEY, ICON_LAYOUT_PANEL_LEFT,
    ICON_LINK, ICON_MAIL, ICON_RADIO_TOWER, ICON_SHIELD_ALERT, ICON_USER_COG, ICON_USER_PLUS,
    ICON_USERS,
};

/// Where the root's menu is hit, in its surface's pixels: its button at the window's top
/// right, and the menu it opens while it shows.
pub(super) fn menu_area(menu: &BeuiSurface, window: Window) -> Area {
    let scale = window.pixels_per_point;
    let right = window.width * scale;
    let button = Rect::from_min_max(
        Pos2::new(right - MENU_BUTTON * scale, 0.0),
        Pos2::new(right, MENU_BUTTON * scale),
    );
    match menu.overlay_area() {
        Area::Region(mut rects) => {
            rects.push(button);
            Area::Region(rects)
        }
        _ => Area::Region(vec![button]),
    }
}

/// What its status row says of how the host's changes sync, as be-host's menu says it.
fn sync_label(sync: &SyncState) -> &'static str {
    match sync {
        SyncState::Saved => "All changes saved",
        SyncState::Connecting => "Connecting…",
        SyncState::Syncing => "Syncing changes…",
        SyncState::Offline(_) => "Offline",
        SyncState::Failed(_) => "Changes could not sync",
    }
}

fn sync_detail(sync: &SyncState) -> String {
    match sync {
        SyncState::Saved | SyncState::Connecting | SyncState::Syncing => String::new(),
        SyncState::Offline(0) => "Nothing waiting to sync".to_owned(),
        SyncState::Offline(1) => "1 change kept on this device to sync".to_owned(),
        SyncState::Offline(waiting) => format!("{waiting} changes kept on this device to sync"),
        SyncState::Failed(error) => error.clone(),
    }
}

fn sync_glyph(sync: &SyncState) -> Glyph {
    match sync {
        SyncState::Saved => ICON_CLOUD_CHECK,
        SyncState::Connecting | SyncState::Syncing => ICON_CLOUD_SYNC,
        SyncState::Offline(_) => ICON_CLOUD_OFF,
        SyncState::Failed(_) => ICON_CLOUD_ALERT,
    }
}

/// The root's menu, as be-host's own organization menu shows it: how its changes sync; its
/// unsent edits while there are some; why the escrow key is not trusted yet, while it is not;
/// the blocks it keeps hosting, each to stop; contacts and opening a share link; inviting a
/// member to a company's organization, and its members and invitations for an administrator;
/// agents, sessions, switching organization or account, and a new recovery phrase; and the
/// statistics panel. be-host's About, Close and its other panels and inspector are be-host's
/// own until it hands them over (plans/host_as_platform_adapter.md, 4g7g).
#[component]
pub(super) fn HostMenu(
    core: Signal<CoreView>,
    saving: Signal<SavingState>,
    want: Callback<Asking>,
    statistics: Callback<()>,
) -> NodeId {
    let sync = create_memo(move || saving.with(|saving| saving.sync.clone()));
    let sync_label = create_memo(move || sync.with(sync_label).to_owned());
    let sync_detail = create_memo(move || sync.with(sync_detail));
    let sync_glyph = create_memo(move || sync.with(sync_glyph));
    let unsaved = create_memo(move || saving.with(|saving| saving.unsaved));
    let unsent = create_memo(move || core.with(|view| view.unsent.len()));
    let has_unsent = create_memo(move || unsent.get() > 0);
    let unsent_detail = create_memo(move || match unsent.get() {
        1 => "1 block".to_owned(),
        count => format!("{count} blocks"),
    });
    let organization = create_memo(move || core.with(|view| view.organization.clone()));
    let signed_in_as = create_memo(move || core.with(|view| view.signed_in_as.clone()));
    let company = create_memo(move || core.with(|view| view.company));
    let administrator = create_memo(move || core.with(|view| view.administrator));
    let escrow_warning =
        create_memo(move || core.with(|view| view.escrow_warning.clone().unwrap_or_default()));
    let warned = create_memo(move || !escrow_warning.with(String::is_empty));
    let hosting = create_memo(move || core.with(|view| view.hosting.clone()));
    let items = view! {
        <MenuItem label={sync_label} glyph={sync_glyph} detail={sync_detail} disabled=true />
        <Show value={has_unsent}>
            <MenuItem
                label="Unsent edits"
                glyph=ICON_ARCHIVE_RESTORE
                detail={unsent_detail}
                on_click={move || want.call(Asking::Unsent)}
            />
        </Show>
        <Show value={warned}>
            <MenuItem
                label="Escrow key not verified"
                glyph=ICON_SHIELD_ALERT
                detail={escrow_warning}
                disabled=true
            />
        </Show>
        <ForEach value={hosting} key={|kept: &HostedBlock| kept.block}>|kept: ReadSignal<HostedBlock>|
            <MenuItem
                label={move || kept.with(|kept| format!("Stop hosting {}", kept.name))}
                glyph=ICON_RADIO_TOWER
                detail="Kept hosting here"
                on_click={move || ask_core(CoreCommand::StopHosting(kept.with_untracked(|kept| kept.block)))}
            />
        </ForEach>
        <MenuItem
            label="Contacts"
            glyph=ICON_CONTACT
            separated=true
            on_click={move || want.call(Asking::Contacts)}
        />
        <MenuItem
            label="Open a share link"
            glyph=ICON_LINK
            on_click={move || want.call(Asking::ShareLinks)}
        />
        <Show value={company}>
            <MenuItem
                label="Invite member"
                glyph=ICON_USER_PLUS
                detail={organization}
                on_click={move || want.call(Asking::Invite)}
            />
        </Show>
        <Show value={administrator}>
            <MenuItem
                label="Members"
                glyph=ICON_USERS
                on_click={move || want.call(Asking::Members)}
            />
            <MenuItem
                label="Invitations"
                glyph=ICON_MAIL
                on_click={move || want.call(Asking::Invitations)}
            />
        </Show>
        <MenuItem label="Agents" glyph=ICON_BOT on_click={move || want.call(Asking::Agents)} />
        <MenuItem
            label="Sessions and profiles"
            glyph=ICON_LAYOUT_PANEL_LEFT
            on_click={|| ask_core(CoreCommand::ChooseSession)}
        />
        <MenuItem
            label="Switch organization"
            glyph=ICON_ARROW_LEFT_RIGHT
            on_click={|| ask_core(CoreCommand::SwitchOrganization)}
        />
        <MenuItem
            label="New recovery phrase"
            glyph=ICON_KEY
            on_click={|| ask_core(CoreCommand::NewRecoveryPhrase)}
        />
        <MenuItem
            label="Manage accounts"
            glyph=ICON_USER_COG
            detail={signed_in_as}
            disabled={unsaved}
            on_click={|| ask_core(CoreCommand::SwitchAccount)}
        />
        <MenuItem
            label=STATISTICS_TITLE
            glyph=ICON_GAUGE
            separated=true
            on_click={move || statistics.call(())}
        />
    };
    view! {
        <Frame width=Length::FILL padding=Space::Sm>
            <List direction=Direction::Horizontal spacing=Space::None justify=Justify::End>
                <MenuButton label="Menu" glyph=ICON_MENU icon_only=true arrow=false items={items} />
            </List>
        </Frame>
    }
}
