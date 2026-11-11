use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::icons::{ICON_ARROW_DOWN_LEFT, ICON_ARROW_UP_RIGHT, ICON_BELL, ICON_EYE};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::{AccessLevel, BlockParent, NotifyMode, label_glyph};

use super::panel::{Refs, TabInfo};
use super::tab::TabItem;
use super::workspace::Workspace;

const DETAILS: &str = "editor.details";

pub(crate) fn with_block_menu(
    workspace: Workspace,
    info: &TabInfo,
    mut nodes: Vec<MenuNode>,
) -> Vec<MenuNode> {
    let mut added = vec![MenuNode::Separator];
    added.extend(related(
        workspace,
        "workspace.references",
        "References",
        ICON_ARROW_UP_RIGHT,
        &info.references,
    ));
    added.extend(related(
        workspace,
        "workspace.backrefs",
        "Referenced by",
        ICON_ARROW_DOWN_LEFT,
        &info.backrefs,
    ));
    added.push(access(workspace, info));
    added.extend(notifications(workspace, info));
    added.push(MenuNode::Separator);
    let at = nodes
        .iter()
        .position(|node| node.id() == Some(DETAILS))
        .unwrap_or(nodes.len());
    nodes.splice(at..at, added);
    tidy_menu(nodes)
}

fn related(
    workspace: Workspace,
    id: &str,
    title: &str,
    glyph: Glyph,
    refs: &Refs,
) -> Option<MenuNode> {
    refs.state.as_ref().ok()?;
    let types = workspace.types();
    let items = refs
        .list
        .iter()
        .map(|reference| {
            let label = reference.label(types.as_ref());
            let glyph = label_glyph(&label);
            let name = match reference.deleted {
                true => format!("{} (in Recently Deleted)", label.name),
                false => label.name,
            };
            let item = TabItem::new(reference.id, reference.block_type);
            let via = reference.parent.and_then(BlockParent::block);
            let open = Action::new(format!("{id}.{}", reference.id), name, move || {
                workspace.open(item, via);
            })
            .glyph(glyph)
            .detached();
            MenuNode::action(&open)
        })
        .collect();
    let title = format!("{title} ({})", refs.list.len());
    Some(MenuNode::submenu(id, title, glyph, items))
}

fn access(workspace: Workspace, info: &TabInfo) -> MenuNode {
    let id = info.item.id;
    let levels = [
        (AccessLevel::Manage, "manage", "Managing"),
        (AccessLevel::Edit, "edit", "Editing"),
        (AccessLevel::View, "view", "Viewing"),
        (AccessLevel::KnowExists, "none", "No access"),
    ];
    let items = levels
        .into_iter()
        .map(|(level, key, label)| {
            let chosen = Action::new(format!("workspace.access.{key}"), label, move || {
                if workspace.access(id) != level {
                    workspace.simulate(id, level);
                }
            })
            .radio(chosen(info.access) == level)
            .enabled(level <= info.ceiling)
            .detached();
            MenuNode::action(&chosen)
        })
        .collect();
    MenuNode::submenu(
        "workspace.access",
        format!("Access: {}", wording(info.access)),
        ICON_EYE,
        items,
    )
}

/// How the account is notified about the block: every notification, only those that mention
/// it, or none. Read here rather than in `TabInfo`, so the host's answer changes the menu and
/// nothing else about the tab.
fn notifications(workspace: Workspace, info: &TabInfo) -> Option<MenuNode> {
    if !info.access.can_view() {
        return None;
    }
    let id = info.item.id;
    let mode = workspace.notify_mode(id)?;
    let modes = [
        (NotifyMode::All, "all", "All"),
        (NotifyMode::Mentions, "mentions", "Mentions only"),
        (NotifyMode::Off, "off", "Off"),
    ];
    let items = modes
        .into_iter()
        .map(|(choice, key, label)| {
            let chosen = Action::new(format!("workspace.notify.{key}"), label, move || {
                workspace.set_notify_mode(id, choice);
            })
            .radio(mode == choice)
            .detached();
            MenuNode::action(&chosen)
        })
        .collect();
    let title = match mode {
        NotifyMode::All => "Notifications: All",
        NotifyMode::Mentions => "Notifications: Mentions only",
        NotifyMode::Off => "Notifications: Off",
    };
    Some(MenuNode::submenu(
        "workspace.notify",
        title,
        ICON_BELL,
        items,
    ))
}

fn chosen(access: AccessLevel) -> AccessLevel {
    match access {
        AccessLevel::None => AccessLevel::KnowExists,
        access => access,
    }
}

fn wording(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Manage => "Managing",
        AccessLevel::Edit => "Editing",
        AccessLevel::View => "Viewing",
        AccessLevel::KnowExists | AccessLevel::None => "No access",
    }
}
