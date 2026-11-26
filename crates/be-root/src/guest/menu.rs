//! The root's menu, which opens what be-host's organization menu does.

use super::*;
use be_sdk_api::HostPanel;
use beui::icons::{
    ICON_ARCHIVE_RESTORE, ICON_ARROW_LEFT_RIGHT, ICON_BOT, ICON_BUG, ICON_CLOUD_ALERT,
    ICON_CLOUD_CHECK, ICON_CLOUD_OFF, ICON_CLOUD_SYNC, ICON_CONTACT, ICON_GAUGE, ICON_INFO,
    ICON_KEY, ICON_LAYERS, ICON_LAYOUT_PANEL_LEFT, ICON_LINK, ICON_MAIL, ICON_POWER, ICON_PUZZLE,
    ICON_RADIO_TOWER, ICON_SHIELD_ALERT, ICON_USER_COG, ICON_USER_PLUS, ICON_USERS,
};
use beui::{Placement, ScrollPosition, Vec2};

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

/// The glyph of a host panel's row, as be-host's menu draws it.
fn panel_glyph(panel: HostPanel) -> Glyph {
    match panel {
        HostPanel::BlockStack => ICON_LAYERS,
        HostPanel::Performance => ICON_GAUGE,
        HostPanel::Apps => ICON_PUZZLE,
        HostPanel::Menu => ICON_MENU,
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

/// How the root's menu shows: behind its button, or as the host menu panel an editor places
/// (`HostPanel::Menu`), which says what it wants of its place.
#[derive(Clone, Copy)]
pub(super) enum MenuShape {
    /// Behind its own button, at the window's top right.
    Button,
    /// Its rows laid out in its place, a popover's: the size they want goes to `wants`, and a
    /// pick or Escape to `done`, which closes it.
    Rows {
        wants: Callback<Vec2>,
        done: Callback<()>,
    },
    /// A sheet it draws itself over its place, rising from the bottom, as be-host's own does on
    /// a phone: a pick, Escape or a swipe down goes to `done`.
    Sheet { done: Callback<()> },
}

/// The root's menu, as be-host's own organization menu shows it: how its changes sync; its
/// unsent edits while there are some; why the escrow key is not trusted yet, while it is not;
/// the blocks it keeps hosting, each to stop; contacts and opening a share link; inviting a
/// member to a company's organization, and its members and invitations for an administrator;
/// agents, sessions, switching organization or account, and a new recovery phrase; About, and
/// Close be-host where the host can close; every host panel, and the host's inspector. Each row
/// has be-host's test id.
#[component]
pub(super) fn HostMenu(
    core: Signal<CoreView>,
    saving: Signal<SavingState>,
    want: Callback<Asking>,
    /// Shows one of the host's panels, as be-host's menu does: in the session's editor, which
    /// places it, and the root draws it there.
    panel: Callback<HostPanel>,
    shape: MenuShape,
) -> NodeId {
    let done = match shape {
        MenuShape::Button => Callback::empty(),
        MenuShape::Rows { done, .. } | MenuShape::Sheet { done } => done,
    };
    // A pick closes the panel it was made in, as be-host's does.
    let want = Callback::new(move |asking: Asking| {
        want.call(asking);
        done.call(());
    });
    let panel = Callback::new(move |shown: HostPanel| {
        panel.call(shown);
        done.call(());
    });
    let command = move |command: CoreCommand| {
        ask_core(command);
        done.call(());
    };
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
    let organization =
        create_memo(move || core.with(|view| view.organization.clone().unwrap_or_default()));
    // Who is signed in, as be-host's menu words it: the account open now.
    let signed_in_as = create_memo(move || {
        core.with(|view| {
            view.accounts
                .iter()
                .find(|account| account.current)
                .map(|account| format!("Signed in as {}", account.name))
                .unwrap_or_default()
        })
    });
    let company = create_memo(move || core.with(|view| view.company));
    let served = create_memo(move || core.with(|view| !view.local));
    let administrator = create_memo(move || core.with(|view| view.administrator));
    let can_close = create_memo(move || core.with(|view| view.can_close));
    let escrow_warning =
        create_memo(move || core.with(|view| view.escrow_warning.clone().unwrap_or_default()));
    let warned = create_memo(move || !escrow_warning.with(String::is_empty));
    let hosting = create_memo(move || core.with(|view| view.hosting.clone()));
    let items = view! {
        <MenuItem
            @test_id={"host.menu.status".to_owned()}
            label={sync_label}
            glyph={sync_glyph}
            detail={sync_detail}
            disabled=true
        />
        <Show value={has_unsent}>
            <MenuItem
                @test_id={"host.menu.unsent".to_owned()}
                label="Unsent edits"
                glyph=ICON_ARCHIVE_RESTORE
                detail={unsent_detail}
                on_click={move || want.call(Asking::Unsent)}
            />
        </Show>
        <Show value={warned}>
            <MenuItem
                @test_id={"host.menu.escrow-warning".to_owned()}
                label="Escrow key not verified"
                glyph=ICON_SHIELD_ALERT
                detail={escrow_warning}
                disabled=true
            />
        </Show>
        <ForEach
            value={hosting}
            key={|kept: &HostedBlock| kept.block}
        >|kept: ReadSignal<HostedBlock>|
            <MenuItem
                @test_id={kept.with_untracked(|kept| format!("host.menu.hosting.{}", be_lib_ui_types::clipboard::Uuid::from_bytes(kept.block)))}
                label={move || kept.with(|kept| format!("Stop hosting {}", kept.name))}
                glyph=ICON_RADIO_TOWER
                detail={move || kept.with(|kept| match kept.now {
                    true => "Hosting here now".to_owned(),
                    false => "Kept here, standing by".to_owned(),
                })}
                on_click={move || command(CoreCommand::StopHosting(kept.with_untracked(|kept| kept.block)))}
            />
        </ForEach>
        <Show value={served}>
            <MenuItem
                label="Contacts"
                glyph=ICON_CONTACT
                @test_id={"host.menu.contacts"}
                separated=true
                on_click={move || want.call(Asking::Contacts)}
            />
            <MenuItem
                label="Open a share link"
                glyph=ICON_LINK
                @test_id={"host.menu.share-link"}
                on_click={move || want.call(Asking::ShareLinks)}
            />
        </Show>
        <Show value={company}>
            <MenuItem
                label="Invite member"
                glyph=ICON_USER_PLUS
                @test_id={"host.menu.invite"}
                separated={move || !served.get()}
                detail={organization}
                on_click={move || want.call(Asking::Invite)}
            />
        </Show>
        <Show value={administrator}>
            <MenuItem
                label="Members"
                glyph=ICON_USERS
                @test_id={"host.menu.members"}
                separated={move || !served.get() && !company.get()}
                on_click={move || want.call(Asking::Members)}
            />
            <MenuItem
                label="Invitations"
                glyph=ICON_MAIL
                @test_id={"host.menu.invitations"}
                on_click={move || want.call(Asking::Invitations)}
            />
        </Show>
        <MenuItem
            label="Agents"
            glyph=ICON_BOT
            @test_id={"host.menu.agents"}
            separated={move || !served.get() && !company.get() && !administrator.get()}
            on_click={move || want.call(Asking::Agents)}
        />
        <MenuItem
            @test_id={"host.menu.switch-session".to_owned()}
            label="Sessions and profiles"
            glyph=ICON_LAYOUT_PANEL_LEFT
            on_click={move || command(CoreCommand::ChooseSession)}
        />
        <MenuItem
            @test_id={"host.menu.switch-organization".to_owned()}
            label="Switch organization"
            glyph=ICON_ARROW_LEFT_RIGHT
            on_click={move || command(CoreCommand::SwitchOrganization)}
        />
        <MenuItem
            @test_id={"host.menu.recovery-phrase".to_owned()}
            label="New recovery phrase"
            glyph=ICON_KEY
            on_click={move || command(CoreCommand::NewRecoveryPhrase)}
        />
        <MenuItem
            @test_id={"host.menu.accounts".to_owned()}
            label="Manage accounts"
            glyph=ICON_USER_COG
            detail={signed_in_as}
            disabled={unsaved}
            on_click={move || command(CoreCommand::SwitchAccount)}
        />
        <MenuItem
            @test_id={"host.menu.about".to_owned()}
            label="About"
            glyph=ICON_INFO
            on_click={move || want.call(Asking::About)}
        />
        <Show value={can_close}>
            <MenuItem
                @test_id={"host.menu.close".to_owned()}
                label="Close be-host"
                glyph=ICON_POWER
                on_click={move || command(CoreCommand::Close)}
            />
        </Show>
        <ForEach value={HostPanel::ALL.to_vec()}>|shown: HostPanel|
            <MenuItem
                @test_id={format!("host.menu.panel.{shown:?}")}
                label={shown.title().to_owned()}
                glyph={panel_glyph(shown)}
                separated={shown == HostPanel::ALL[0]}
                on_click={move || panel.call(shown)}
            />
        </ForEach>
        <MenuItem
            @test_id={"host.menu.inspector".to_owned()}
            label="Inspector"
            glyph=ICON_BUG
            on_click={move || command(CoreCommand::OpenInspector)}
        />
    };
    match shape {
        MenuShape::Button => view! {
            <Frame width=Length::FILL padding=Space::Sm>
                <List direction=Direction::Horizontal spacing=Space::None justify=Justify::End>
                    <MenuButton
                        label="Menu"
                        glyph=ICON_MENU
                        icon_only=true
                        arrow=false
                        items={items}
                    />
                </List>
            </Frame>
        },
        MenuShape::Sheet { done } => view! {
            <MenuPopup
                items
                anchor={OverlayAnchor::Point(Pos2::ZERO)}
                placement=Placement::Fill
                open={Binding::new(true, move |open: bool| {
                    if !open {
                        done.call(());
                    }
                })}
                sheet=true
            />
        },
        MenuShape::Rows { wants, done } => {
            // The popover starts at a size of its own and grows to what the rows want, which
            // they measure against what that start gave them: that plus what their scroll holds
            // beyond its viewport, as be-host's own menu panel reports it. Only a window too
            // short for them scrolls, in the menu's own scroll.
            let scrolled = create_signal(ScrollPosition::ZERO);
            let rows = view! {
                <MenuRows active=true items on_scroll={move |position| scrolled.set(position)} />
            };
            let held = NodeRef::new();
            held.fill(rows);
            on_key(
                KeyAt::Node(held),
                KeyPhase::Bubble,
                KeyReveal::Focus,
                move |event| {
                    let escaped = event
                        .press()
                        .is_some_and(|press| press.pressed && press.key == Key::Escape);
                    if escaped {
                        done.call(());
                    }
                    escaped
                },
            );
            let measured = measure(rows);
            // One memo of both, so the popover hears one size per change of either.
            let wanted = create_memo(move || {
                let measured = measured.get()?;
                let beyond =
                    scrolled.with(|scrolled| (scrolled.content - scrolled.viewport).max(0.0));
                Some(measured.rect.size() + vec2(0.0, beyond))
            });
            create_effect(move || {
                if let Some(size) = wanted.get() {
                    wants.call(size);
                }
            });
            view! {
                <Frame align={Align2::START}>{rows}</Frame>
            }
        }
    }
}
