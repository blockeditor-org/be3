use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::reactive::{
    Func, Memo, clone, component, component_rect, create_effect, create_memo, create_signal,
    on_cleanup, untrack, view,
};
use beui::styled::DockArea;
use beui::unstyled::{DockState, DockTree, DockTreeEntry, GroupId, TabId, Tree};
use beui::{NodeId, Rect, pos2, vec2};
use block_plugin_api::{PaneId, PaneLayout, PaneTree};
use block_ui::panes::{dock_tree_with, pane_tree_with};

use super::debug::{
    ClientPanel, DebugCommand, DebugWindow, PerformancePanel, PluginsPanel, VersionPanel,
};
use super::dialogs::{AboutPanel, InvitePanel};
use super::{AppViewStore, UiCommand, send};
use crate::surfaces::{HostSurface, SurfaceId};

const WORKSPACE: TabId = TabId::new(1);
const PANE_TABS: u64 = 1 << 40;
const HEADLESS_OFFSET: f32 = 64.0;

fn pane_tab(pane: PaneId) -> TabId {
    TabId::new(PANE_TABS + pane.0)
}

fn tab_pane(tab: TabId) -> Option<PaneId> {
    tab.value().checked_sub(PANE_TABS).map(PaneId)
}

#[derive(Clone, PartialEq)]
struct Arranged {
    tree: PaneTree,
    detached: Vec<PaneId>,
    focused: Option<PaneId>,
}

#[derive(Default)]
struct Docked {
    group: Cell<Option<GroupId>>,
    arrangement: Cell<u64>,
    synced: RefCell<Option<Arranged>>,
}

fn pane_tabs(state: &DockState) -> Vec<TabId> {
    state
        .all_tabs()
        .into_iter()
        .filter(|tab| tab_pane(*tab).is_some())
        .collect()
}

fn outside(state: &DockState, group: GroupId) -> Vec<TabId> {
    let inside = state.group_tabs(group);
    pane_tabs(state)
        .into_iter()
        .filter(|tab| !inside.contains(tab))
        .collect()
}

fn arranged(state: &DockState, group: GroupId) -> Arranged {
    let tree = state.tree(Tree::Group(group)).unwrap_or_default();
    Arranged {
        tree: pane_tree_with(&tree, &tab_pane),
        detached: outside(state, group)
            .into_iter()
            .filter_map(tab_pane)
            .collect(),
        focused: state.focused_tab().and_then(tab_pane),
    }
}

fn with_tabs(tree: DockTree, added: &[TabId]) -> DockTree {
    if added.is_empty() {
        return tree;
    }
    match tree {
        DockTree::Tabs {
            mut entries,
            active,
            vertical,
            sidebar,
        } => {
            entries.extend(added.iter().map(|tab| DockTreeEntry::Tab(*tab)));
            DockTree::Tabs {
                entries,
                active,
                vertical,
                sidebar,
            }
        }
        DockTree::Split {
            direction,
            fraction,
            first,
            second,
        } => DockTree::Split {
            direction,
            fraction,
            first: Box::new(with_tabs(*first, added)),
            second,
        },
    }
}

fn apply(state: &mut DockState, docked: &Docked, layout: Option<&PaneLayout>) {
    let Some(layout) = layout else {
        if let Some(group) = docked.group.take() {
            for tab in pane_tabs(state) {
                state.remove(tab);
            }
            state.unpin(group);
            docked.synced.replace(None);
        }
        if !state.contains(WORKSPACE)
            && let Some(leaf) = state.leaves(state.main()).first().copied()
        {
            state.insert(leaf, 0, WORKSPACE);
        }
        return;
    };
    let tree = dock_tree_with(&layout.tree, &pane_tab).unwrap_or_default();
    let group = match docked.group.get() {
        Some(group) => group,
        None => {
            let (leaf, index) = match state.find(WORKSPACE) {
                Some(position) => (position.leaf, position.index),
                None => (state.leaves(state.main())[0], 0),
            };
            let group = state.insert_pinned_group(leaf, index, &DockTree::default());
            state.remove(WORKSPACE);
            docked.group.set(Some(group));
            group
        }
    };
    if layout.arrangement >= docked.arrangement.get() {
        let listed: Vec<TabId> = layout
            .panes
            .iter()
            .map(|info| pane_tab(info.pane))
            .collect();
        for tab in pane_tabs(state) {
            if !listed.contains(&tab) {
                state.remove(tab);
            }
        }
        let detached = outside(state, group);
        let tree = tree.without(&detached);
        let placed = tree.tabs();
        let missing: Vec<TabId> = listed
            .into_iter()
            .filter(|tab| !placed.contains(tab) && !detached.contains(tab))
            .collect();
        state.set_tree(Tree::Group(group), &with_tabs(tree, &missing));
    }
    docked.synced.replace(Some(arranged(state, group)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Debug(DebugWindow),
    Invite,
    About,
}

impl Tool {
    const ALL: [Tool; 6] = [
        Tool::Debug(DebugWindow::Client),
        Tool::Debug(DebugWindow::Performance),
        Tool::Debug(DebugWindow::Plugins),
        Tool::Debug(DebugWindow::Version),
        Tool::Invite,
        Tool::About,
    ];

    fn tab(self) -> TabId {
        let index = Self::ALL
            .iter()
            .position(|tool| *tool == self)
            .unwrap_or_default();
        TabId::new(2 + index as u64)
    }

    fn of(tab: TabId) -> Option<Tool> {
        Self::ALL.into_iter().find(|tool| tool.tab() == tab)
    }

    fn title(self) -> &'static str {
        match self {
            Tool::Debug(DebugWindow::Client) => "Block Stack State",
            Tool::Debug(DebugWindow::Performance) => "Performance",
            Tool::Debug(DebugWindow::Plugins) => "Plugins",
            Tool::Debug(DebugWindow::Version) => "App Version",
            Tool::Invite => "Invite member",
            Tool::About => "About",
        }
    }

    fn window(self) -> Rect {
        let (position, size) = match self {
            Tool::Debug(DebugWindow::Client) => (pos2(60.0, 60.0), vec2(760.0, 600.0)),
            Tool::Debug(DebugWindow::Performance) => (pos2(120.0, 100.0), vec2(520.0, 420.0)),
            Tool::Debug(DebugWindow::Plugins) => (pos2(150.0, 90.0), vec2(560.0, 440.0)),
            Tool::Debug(DebugWindow::Version) => (pos2(180.0, 110.0), vec2(640.0, 480.0)),
            Tool::Invite => (pos2(120.0, 96.0), vec2(380.0, 360.0)),
            Tool::About => (pos2(160.0, 120.0), vec2(420.0, 230.0)),
        };
        Rect::from_min_size(position, size)
    }

    fn open(self, view: &AppViewStore) -> Memo<bool> {
        let view = view.clone();
        create_memo(move || match self {
            Tool::Debug(window) => {
                let debug = view.debug.get();
                match window {
                    DebugWindow::Client => debug.client.is_some(),
                    DebugWindow::Performance => debug.performance.is_some(),
                    DebugWindow::Plugins => debug.plugins.is_some(),
                    DebugWindow::Version => debug.version.is_some(),
                }
            }
            Tool::Invite => view.invite.get().is_some(),
            Tool::About => view.about.get(),
        })
    }

    fn close(self) {
        send(match self {
            Tool::Debug(window) => UiCommand::Debug(DebugCommand::Close(window)),
            Tool::Invite => UiCommand::CloseInvite,
            Tool::About => UiCommand::About(false),
        });
    }
}

#[component]
pub(super) fn WorkspaceDock(view: AppViewStore) -> NodeId {
    let (state, set_state) = create_signal(DockState::new([WORKSPACE]));
    let docked = Rc::new(Docked::default());
    let panes = create_memo(clone!(view -> move || view.panes.get().layout));
    create_effect(clone!(set_state docked panes -> move || {
        let layout = panes.get();
        untrack(|| set_state.update(|state| apply(state, &docked, layout.as_ref())));
    }));
    let shown = create_memo(clone!(view -> move || view.panes.get().shown));
    create_effect(clone!(set_state -> move || {
        if let Some((_, pane)) = shown.get() {
            untrack(|| set_state.update(|state| state.show(pane_tab(pane))));
        }
    }));
    create_effect(clone!(state docked -> move || {
        let current = state.get();
        let Some(group) = docked.group.get() else {
            return;
        };
        let now = arranged(&current, group);
        if docked.synced.borrow().as_ref() == Some(&now) {
            return;
        }
        docked.synced.replace(Some(now.clone()));
        let arrangement = docked.arrangement.get() + 1;
        docked.arrangement.set(arrangement);
        send(UiCommand::ArrangePanes {
            arrangement,
            tree: now.tree,
            detached: now.detached,
            focused: now.focused,
        });
    }));
    let area = component_rect();
    create_effect(clone!(panes -> move || {
        let area = area.get();
        let headless = panes.with(Option::is_some).then(|| {
            Rect::from_min_size(
                pos2(area.min.x, -HEADLESS_OFFSET),
                vec2(area.width().max(1.0), 1.0),
            )
        });
        crate::surfaces::set_headless(headless);
    }));
    on_cleanup(|| crate::surfaces::set_headless(None));
    for tool in Tool::ALL {
        let open = tool.open(&view);
        create_effect(clone!(set_state -> move || {
            let open = open.get();
            untrack(|| {
                set_state.update(|state| match (open, state.contains(tool.tab())) {
                    (true, false) => {
                        state.open_window(tool.window(), vec![tool.tab()]);
                    }
                    (false, true) => {
                        state.remove(tool.tab());
                    }
                    _ => {}
                });
            });
        }));
    }
    let status = view.status.clone();
    let listed = panes.clone();
    let info = move |pane: PaneId| {
        listed.with(|layout| {
            layout
                .as_ref()
                .and_then(|layout| layout.panes.iter().find(|info| info.pane == pane).cloned())
        })
    };
    let titled = info.clone();
    let named = status.clone();
    let title = Func::new(move |tab: TabId| match (Tool::of(tab), tab_pane(tab)) {
        (Some(tool), _) => tool.title().to_owned(),
        (None, Some(pane)) => titled(pane).map(|info| info.title).unwrap_or_default(),
        (None, None) => named.get().workspace,
    });
    let grouped = docked.clone();
    let group_title = Func::new(move |group: GroupId| {
        (grouped.group.get() == Some(group)).then(|| status.get().workspace)
    });
    let closable = Func::new(move |tab: TabId| match tab_pane(tab) {
        Some(pane) => info(pane).is_some_and(|info| info.closable),
        None => tab != WORKSPACE,
    });
    view! {
        <DockArea
            state={state}
            title={title}
            group_title={group_title}
            closable={closable}
            on_change={move |next: DockState| set_state.set(next)}
            on_close={|tab: TabId| {
                if let Some(tool) = Tool::of(tab) {
                    tool.close();
                }
                if let Some(pane) = tab_pane(tab) {
                    send(UiCommand::ClosePane(pane));
                }
            }}
        >
            {move |tab: TabId| {
                let view = view.clone();
                let debug = view.debug.clone();
                if let Some(pane) = tab_pane(tab) {
                    return view! {
                        <HostSurface id=SurfaceId::Pane(pane.0) />
                    };
                }
                match Tool::of(tab) {
                    None => view! {
                        <HostSurface id=SurfaceId::Main />
                    },
                    Some(Tool::Debug(DebugWindow::Client)) => {
                        let client = create_memo(move || debug.get().client);
                        view! {
                            <ClientPanel client />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Performance)) => {
                        let performance = create_memo(move || debug.get().performance);
                        view! {
                            <PerformancePanel performance />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Plugins)) => {
                        let plugins = create_memo(move || debug.get().plugins);
                        view! {
                            <PluginsPanel plugins />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Version)) => {
                        let version = create_memo(move || debug.get().version);
                        view! {
                            <VersionPanel version />
                        }
                    }
                    Some(Tool::Invite) => view! {
                        <InvitePanel view />
                    },
                    Some(Tool::About) => view! {
                        <AboutPanel />
                    },
                }
            }}
        </DockArea>
    }
}

#[cfg(test)]
mod tests;
