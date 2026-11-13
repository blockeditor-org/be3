use super::*;
use be_sdk_api::{
    DockChildClose, DockChildHosted, DockDeclare, DockItem, DockItemKind, DockLayout, DockPane,
    DockTab, DockTree, DockViewRequest, ViewRef,
};

const ROOT: EditorInstanceId = EditorInstanceId(1);
const OUTER: EditorInstanceId = EditorInstanceId(2);
const INNER: EditorInstanceId = EditorInstanceId(3);
const OUTER_CHILD: ChildId = ChildId(10);
const INNER_CHILD: ChildId = ChildId(20);
const OUTER_BLOCK: Uuid = Uuid::from_u128(0xa);
const INNER_BLOCK: Uuid = Uuid::from_u128(0xb);

fn pane(depth: u32, key: &str) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Pane(DockPane {
            key: key.into(),
            active: None,
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: None,
            empty: None,
            bare: false,
            hide_empty_bar: false,
        }),
    }
}

fn tab(depth: u32, tab: u64) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Tab(DockTabPath::own(tab)),
    }
}

fn declaration(key: &str, tabs: &[(u64, &str)]) -> DockDeclaration {
    let mut items = vec![pane(0, key)];
    items.extend(tabs.iter().map(|(id, _)| tab(1, *id)));
    DockDeclaration {
        tree: DockTree { items, focus: None },
        tabs: tabs
            .iter()
            .map(|(id, key)| DockTab {
                tab: DockTabPath::own(*id),
                key: (*key).into(),
                title: (*key).into(),
                glyph: String::new(),
                closable: true,
                back_closes: true,
                menu: Vec::new(),
            })
            .collect(),
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}

fn outer() -> DockDeclaration {
    declaration("outer", &[(1, "one"), (2, "nested")])
}

fn inner() -> DockDeclaration {
    declaration("inner", &[(5, "five")])
}

fn declare(instance: EditorInstanceId, declaration: Option<DockDeclaration>) -> DockMessage {
    DockMessage::Declare(DockDeclare {
        view: ViewRef {
            instance,
            view: ViewId::MAIN,
        },
        declaration,
    })
}

fn path(via: &[ChildId], tab: u64) -> DockTabPath {
    DockTabPath {
        via: via.to_vec(),
        tab,
    }
}

fn nested() -> Docks {
    let mut docks = Docks::default();
    docks.register(ROOT, "shell", Some(Uuid::from_u128(1)));
    docks.register(OUTER, "outer", Some(OUTER_BLOCK));
    docks.register(INNER, "inner", Some(INNER_BLOCK));
    docks.placed(
        ROOT,
        &[DockedChild {
            child: OUTER_CHILD,
            tab: 100,
            block: OUTER_BLOCK,
        }],
        None,
    );
    docks.heard("outer", vec![declare(OUTER, Some(outer()))]);
    docks.placed(
        OUTER,
        &[DockedChild {
            child: INNER_CHILD,
            tab: 2,
            block: INNER_BLOCK,
        }],
        None,
    );
    docks.heard("inner", vec![declare(INNER, Some(inner()))]);
    docks
}

fn spliced() -> DockDeclaration {
    let mut spliced = outer();
    let prefix = dock_prefix("nested", INNER_BLOCK.into_bytes());
    assert_eq!(spliced.splice(2, INNER_CHILD, &prefix, &inner()), Ok(true));
    spliced
}

fn updates(docks: &mut Docks, app: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
    let mut queued: Vec<(EditorInstanceId, DockUpdate)> = Vec::new();
    for (to, update) in docks.take(app) {
        let mut held: Vec<DockUpdate> = queued
            .iter()
            .filter(|(at, _)| *at == to)
            .map(|(_, update)| update.clone())
            .collect();
        queued.retain(|(at, _)| *at != to);
        DockUpdate::queue(&mut held, update);
        queued.extend(held.into_iter().map(|update| (to, update)));
    }
    // What a declaration says, not its place in the count (`epochs` has that).
    for (_, update) in &mut queued {
        if let DockUpdate::Child(child) = update {
            child.epoch = 0;
        }
    }
    queued
}

fn hosted(docks: &mut Docks, app: &str, instance: EditorInstanceId) -> DockHosted {
    match updates(docks, app).as_slice() {
        [(to, DockUpdate::Hosted(hosted))] if *to == instance => {
            hosted.hosted.clone().expect("the root hosts it")
        }
        other => panic!("expected one hosted update, got {other:?}"),
    }
}

fn hosted_by_root() -> DockHosted {
    let spliced = spliced();
    DockHosted {
        layout: DockLayout {
            tree: spliced.tree.clone(),
            homes: Vec::new(),
            bars: Vec::new(),
            outside: Vec::new(),
            unlocked: Vec::new(),
            compact: Vec::new(),
            auto_hidden: Vec::new(),
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: Some(path(&[INNER_CHILD], 5)),
        shown: None,
        visible: vec![path(&[], 1), path(&[INNER_CHILD], 5)],
        fullscreen: None,
        mode: be_sdk_api::DockMode::Tiled,
        covers: Vec::new(),
    }
}

/// The root holds its child's tab as `hold`, having heard every declaration sent for it.
fn host(docks: &mut Docks, hold: DockHold) {
    let seen = docks
        .epochs
        .get(&(ROOT, OUTER_CHILD))
        .copied()
        .unwrap_or(0);
    host_seen(docks, hold, seen);
}

/// The root holds its child's tab as `hold`, having heard the declarations up to `seen`.
fn host_seen(docks: &mut Docks, hold: DockHold, seen: u64) {
    docks.heard(
        "shell",
        vec![DockMessage::Host(DockChildHosted {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            hold,
            seen,
        })],
    );
}

mod a_closed_instance_withdraws_its_dock_from_the_root;
mod a_descendants_bar_is_placed_by_the_root_and_its_moves_reach_it;
mod a_docked_child_stays_open_while_its_placer_hosts_or_declares_its_tab;
mod a_docked_child_that_declares_nothing_stays_a_plain_child;
mod a_docked_child_the_root_stops_hosting_stays_docked_until_the_root_places_it;
mod a_docks_own_bars_reach_the_root_and_come_back_to_each_level_as_its_own;
mod a_frame_childs_bars_take_the_place_of_its_placers_and_give_it_back;
mod a_nested_dock_reaches_the_root_spliced_into_its_placers;
mod a_pick_from_a_tabs_menu_reaches_the_owner_two_levels_down;
mod a_placing_sent_before_the_root_heard_a_declaration_leaves_it_awaited_while_placed;
mod a_placing_sent_before_the_root_heard_a_declaration_leaves_it_awaited_while_unplaced;
mod a_request_reaches_the_root_through_each_placer;
mod a_root_that_answers_a_declaration_by_placing_lets_the_child_go_once_unplaced;
mod a_spliced_childs_ask_for_details_reaches_its_placer;
mod a_tab_the_root_closes_is_closed_at_its_owner;
mod messages_about_another_apps_instances_are_ignored;
mod the_root_places_a_descendants_dialog_and_empty_pane_bodies;
mod the_root_places_only_visible_tabs_of_its_docked_descendants;
mod what_the_root_hosts_is_split_into_each_instances_own_tabs;
