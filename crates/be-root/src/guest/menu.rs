//! The root's menu, which opens what be-host's organization menu does.

use super::*;

/// Where the root's menu is hit, in its surface's pixels: its button, and the menu it opens
/// while it shows.
pub(super) fn menu_area(menu: &BeuiSurface) -> Area {
    let [left, top, right, bottom] = MENU_BUTTON;
    let button = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
    match menu.overlay_area() {
        Area::Region(mut rects) => {
            rects.push(button);
            Area::Region(rects)
        }
        _ => Area::Region(vec![button]),
    }
}

/// The root's menu, as be-host's own organization menu is: the block stack's dialogs (unsent
/// edits, contacts, a share link, the organization's invitations, members and agents), its
/// sessions, switching organization or account, a new recovery phrase, locking the session,
/// and the statistics panel.
#[component]
pub(super) fn HostMenu(want: Callback<Asking>, statistics: Callback<()>) -> NodeId {
    let lock = || {
        be_wasm_ops::guest::spawn(async {
            // A host that cannot hear it has stopped, which the root hears from its frames.
            let _ = host::lock_session().await;
        });
    };
    let items = view! {
        <MenuItem label="Unsent edits" on_click={move || want.call(Asking::Unsent)} />
        <MenuItem label="Contacts" on_click={move || want.call(Asking::Contacts)} />
        <MenuItem label="Open a share link" on_click={move || want.call(Asking::ShareLinks)} />
        <MenuItem label="Invitations" on_click={move || want.call(Asking::Invitations)} />
        <MenuItem label="Members" on_click={move || want.call(Asking::Members)} />
        <MenuItem label="Agents" on_click={move || want.call(Asking::Agents)} />
        <MenuItem label="Sessions and profiles" on_click={|| ask_core(CoreCommand::ChooseSession)} />
        <MenuItem label="Switch organization" on_click={|| ask_core(CoreCommand::SwitchOrganization)} />
        <MenuItem label="New recovery phrase" on_click={|| ask_core(CoreCommand::NewRecoveryPhrase)} />
        <MenuItem label="Manage accounts" on_click={|| ask_core(CoreCommand::SwitchAccount)} />
        <MenuItem label="Lock" on_click={lock} />
        <MenuItem label="Performance" on_click={move || statistics.call(())} />
    };
    view! {
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
    }
}
