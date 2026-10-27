use super::*;
use block_plugin_api::{
    DockChildClose, DockChildHosted, DockDeclare, DockItem, DockItemKind, DockLayout, DockPane,
    DockRegionRequest, DockTab, DockTree, RegionRef,
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
            })
            .collect(),
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
        region: RegionRef {
            instance,
            region: EditorRegion::Frame,
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
    );
    docks.heard("outer", vec![declare(OUTER, Some(outer()))]);
    docks.placed(
        OUTER,
        &[DockedChild {
            child: INNER_CHILD,
            tab: 2,
            block: INNER_BLOCK,
        }],
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

fn updates(docks: &mut Docks, plugin: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
    let mut queued: Vec<(EditorInstanceId, DockUpdate)> = Vec::new();
    for (to, update) in docks.take(plugin) {
        let mut held: Vec<DockUpdate> = queued
            .iter()
            .filter(|(at, _)| *at == to)
            .map(|(_, update)| update.clone())
            .collect();
        queued.retain(|(at, _)| *at != to);
        DockUpdate::queue(&mut held, update);
        queued.extend(held.into_iter().map(|update| (to, update)));
    }
    queued
}

fn hosted(docks: &mut Docks, plugin: &str, instance: EditorInstanceId) -> DockHosted {
    match updates(docks, plugin).as_slice() {
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
        },
        tabs: spliced.tabs.iter().map(|tab| tab.tab.clone()).collect(),
        focused: Some(path(&[INNER_CHILD], 5)),
        shown: None,
        visible: vec![path(&[], 1), path(&[INNER_CHILD], 5)],
        fullscreen: None,
        stacked: false,
    }
}

fn host(docks: &mut Docks, hosted: Option<DockHosted>) {
    docks.heard(
        "shell",
        vec![DockMessage::Host(DockChildHosted {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            hosted,
        })],
    );
}

#[test]
fn a_nested_dock_reaches_the_root_spliced_into_its_placers() {
    let mut docks = nested();
    assert!(docks.docked(OUTER));
    assert!(docks.docked(INNER));
    assert!(!docks.docked(ROOT));
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(spliced()),
            })
        )]
    );
    assert!(updates(&mut docks, "outer").is_empty());
    assert!(updates(&mut docks, "inner").is_empty());
}

#[test]
fn what_the_root_hosts_is_split_into_each_instances_own_tabs() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    host(&mut docks, Some(hosted_by_root()));
    let outer = hosted(&mut docks, "outer", OUTER);
    assert_eq!(outer.layout.tree, self::outer().tree);
    assert_eq!(outer.visible, [path(&[], 1)]);
    assert_eq!(outer.focused, Some(path(&[], 2)));
    assert!(outer.tabs.contains(&path(&[], 2)));
    let inner = hosted(&mut docks, "inner", INNER);
    assert_eq!(inner.layout.tree, self::inner().tree);
    assert_eq!(inner.visible, [path(&[], 5)]);
    assert_eq!(inner.tabs, [path(&[], 5)]);
}

#[test]
fn the_root_places_only_visible_tabs_of_its_docked_descendants() {
    let mut docks = nested();
    host(&mut docks, Some(hosted_by_root()));
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], 5)),
        Some((INNER_BLOCK, 5))
    );
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD], 1)),
        Some((OUTER_BLOCK, 1))
    );
    assert_eq!(docks.resolve(ROOT, &path(&[OUTER_CHILD], 2)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[ChildId(99)], 1)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[INNER_CHILD], 5)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[], 1)), None);
    assert_eq!(docks.resolve(OUTER, &path(&[INNER_CHILD], 5)), None);
}

#[test]
fn a_request_reaches_the_root_through_each_placer() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.heard(
        "inner",
        vec![DockMessage::Request(DockRegionRequest {
            region: RegionRef {
                instance: INNER,
                region: EditorRegion::Frame,
            },
            request: DockRequest::Show(path(&[], 5)),
        })],
    );
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::ChildRequest(DockChildRequest {
                child: OUTER_CHILD,
                owner: vec![INNER_CHILD],
                request: DockRequest::Show(path(&[INNER_CHILD], 5)),
            })
        )]
    );
}

#[test]
fn a_tab_the_root_closes_is_closed_at_its_owner() {
    let mut docks = nested();
    docks.heard(
        "shell",
        vec![DockMessage::Close(DockChildClose {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            tab: path(&[INNER_CHILD], 5),
        })],
    );
    assert_eq!(
        updates(&mut docks, "inner"),
        [(
            INNER,
            DockUpdate::Close(DockClosedTab {
                region: EditorRegion::Frame,
                tab: path(&[], 5),
            })
        )]
    );
    assert!(updates(&mut docks, "outer").is_empty());
}

#[test]
fn messages_about_another_plugins_instances_are_ignored() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.heard("outer", vec![declare(INNER, None)]);
    docks.heard(
        "outer",
        vec![DockMessage::Close(DockChildClose {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            tab: path(&[], 1),
        })],
    );
    assert!(updates(&mut docks, "shell").is_empty());
    assert!(updates(&mut docks, "outer").is_empty());
}

#[test]
fn a_docked_child_stays_open_while_its_placer_hosts_or_declares_its_tab() {
    let mut docks = nested();
    docks.placed(ROOT, &[]);
    docks.placed(OUTER, &[]);
    assert!(docks.docked(OUTER));
    assert!(docks.docked(INNER));
    host(&mut docks, Some(hosted_by_root()));
    assert!(docks.docked(OUTER));
    updates(&mut docks, "shell");
    updates(&mut docks, "inner");

    docks.heard(
        "outer",
        vec![declare(OUTER, Some(declaration("outer", &[(1, "one")])))],
    );
    assert!(!docks.docked(INNER));
    assert_eq!(
        updates(&mut docks, "inner"),
        [(
            INNER,
            DockUpdate::Hosted(DockHostedRegion {
                region: EditorRegion::Frame,
                hosted: None,
            })
        )]
    );
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(declaration("outer", &[(1, "one")])),
            })
        )]
    );

    host(&mut docks, None);
    assert!(!docks.docked(OUTER));
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: None,
            })
        )]
    );
}

#[test]
fn a_closed_instance_withdraws_its_dock_from_the_root() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.forget(INNER);
    assert!(!docks.docked(INNER));
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(outer()),
            })
        )]
    );
}
