use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{ICON_ARROW_DOWN_LEFT, ICON_ARROW_UP_RIGHT, ICON_EYE};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, label_glyph};

use super::panel::{Info, Refs};
use super::tab::TabItem;
use super::workspace::Workspace;

const DETAILS: &str = "editor.details";

pub(crate) fn with_block_menu(
    workspace: Workspace,
    info: &Info,
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
    let mut listed: Vec<&BlockInfo> = refs.list.iter().collect();
    listed.sort_by_key(|reference| reference.deleted);
    let items = listed
        .into_iter()
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

fn access(workspace: Workspace, info: &Info) -> MenuNode {
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
