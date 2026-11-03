use be_sdk_beui::beui::dock::styled::{DockSnap, TabId};
use be_sdk_beui::beui::icons::{
    ICON_ARROW_LEFT_TO_LINE, ICON_ARROW_RIGHT_TO_LINE, ICON_EYE_OFF, ICON_LAYERS, ICON_MAXIMIZE_2,
    ICON_MINIMIZE_2, ICON_MONITOR, ICON_PANEL_LEFT, ICON_PANEL_RIGHT, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::{Pos2, Rect, Vec2};
use be_sdk_beui::{HostWindowId, ScreenLockedWatch};

use super::window::{tab_window, window_tab};
use super::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cleared {
    Window(HostWindowId),
    Block(TabId),
}

#[derive(Clone, Copy)]
pub struct WindowManager {
    workspace: Workspace,
    areas: Memo<Vec<Rect>>,
    cleared: Signal<Option<Vec<Cleared>>>,
    locked: Memo<bool>,
}

pub struct WindowActions {
    pub close: ActionBuilder,
    pub snap_left: ActionBuilder,
    pub snap_right: ActionBuilder,
    pub maximize: ActionBuilder,
    pub restore: ActionBuilder,
    pub hide: ActionBuilder,
    pub show_desktop: ActionBuilder,
    pub next_display: ActionBuilder,
    pub previous_display: ActionBuilder,
    pub next_of_app: ActionBuilder,
}

fn overlap(left: Rect, right: Rect) -> f32 {
    let shared = left.intersect(right);
    shared.width().max(0.0) * shared.height().max(0.0)
}

fn holder(window: Rect, areas: &[Rect]) -> Option<usize> {
    let mut best = None;
    let mut most = 0.0;
    for (at, area) in areas.iter().enumerate() {
        let shared = overlap(window, *area);
        if shared > most {
            best = Some(at);
            most = shared;
        }
    }
    best.or_else(|| (!areas.is_empty()).then_some(0))
}

fn moved_onto(window: Rect, from: Rect, to: Rect) -> Pos2 {
    let along = |offset: f32, length: f32| match length > 0.0 {
        true => offset / length,
        false => 0.0,
    };
    let offset = window.min - from.min;
    let wanted = to.min
        + Vec2::new(
            along(offset.x, from.width()) * to.width(),
            along(offset.y, from.height()) * to.height(),
        );
    let room = to.max - window.size();
    Pos2::new(
        wanted.x.min(room.x).max(to.min.x),
        wanted.y.min(room.y).max(to.min.y),
    )
}

impl WindowManager {
    pub fn new(workspace: Workspace, areas: Memo<Vec<Rect>>) -> Self {
        Self {
            workspace,
            areas,
            cleared: create_signal(None),
            locked: workspace.editor().watch::<ScreenLockedWatch>(),
        }
    }

    pub fn workspace(&self) -> Workspace {
        self.workspace
    }

    pub fn areas(&self) -> Vec<Rect> {
        let areas = self.areas.get();
        match areas.is_empty() {
            true => vec![self.workspace.layout().area()],
            false => areas,
        }
    }

    pub fn focused(&self) -> Option<TabId> {
        let focused = self.workspace.layout().focused()?;
        let shown = self.workspace.shown_window_tabs();
        if shown.contains(&focused) {
            return Some(focused);
        }
        let dialog = tab_window(focused)?;
        let root = self.workspace.entries().with(|entries| {
            let mut id = dialog;
            for _ in 0..entries.len() {
                match entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .and_then(|entry| entry.parent)
                {
                    Some(parent) => id = parent,
                    None => break,
                }
            }
            id
        });
        let root = self.workspace.window_tab_of(root);
        shown.contains(&root).then_some(root)
    }

    fn clearable(&self) -> Vec<TabId> {
        self.workspace
            .shown_window_tabs()
            .into_iter()
            .filter(|tab| tab_window(*tab).is_none() || self.workspace.program_window(*tab).is_some())
            .collect()
    }

    pub fn display_of(&self, tab: TabId) -> Option<usize> {
        let rect = self.workspace.layout().window_rect(&tab)?;
        holder(rect, &self.areas())
    }

    fn area_of(&self, tab: TabId) -> Option<Rect> {
        let display = self.display_of(tab)?;
        self.areas().get(display).copied()
    }

    pub fn close(&self, tab: TabId) {
        let dialog = self.workspace.program_window(tab).and_then(|window| {
            self.workspace.entries().with_untracked(|entries| {
                let root_of = |mut id: HostWindowId| {
                    for _ in 0..entries.len() {
                        match entries
                            .iter()
                            .find(|entry| entry.id == id)
                            .and_then(|entry| entry.parent)
                        {
                            Some(parent) => id = parent,
                            None => break,
                        }
                    }
                    id
                };
                entries
                    .iter()
                    .find(|entry| {
                        entry.focused && entry.parent.is_some() && root_of(entry.id) == window
                    })
                    .map(|entry| entry.id)
            })
        });
        match dialog {
            Some(dialog) => self.workspace.close(window_tab(dialog)),
            None => self.workspace.close(tab),
        }
    }

    pub fn hide(&self, tab: TabId) {
        let next = untrack(|| self.next_after(tab));
        let layout = self.workspace.layout();
        let block = self.workspace.program_window(tab).is_none();
        if block && let Some(next) = next {
            layout.show(&next);
        }
        self.workspace.hide(tab);
        if !block && let Some(next) = next {
            layout.show(&next);
        }
    }

    fn next_after(&self, tab: TabId) -> Option<TabId> {
        let shown = self.workspace.shown_window_tabs();
        self.workspace
            .layout()
            .recent()
            .into_iter()
            .find(|other| *other != tab && shown.contains(other))
    }

    pub fn snap(&self, tab: TabId, snap: DockSnap) {
        if let Some(area) = untrack(|| self.area_of(tab)) {
            self.workspace.layout().snap_window(&tab, snap, area);
        }
    }

    pub fn restore(&self, tab: TabId) -> bool {
        self.workspace.layout().unsnap_window(&tab)
    }

    pub fn move_to_display(&self, tab: TabId, display: usize) {
        let layout = self.workspace.layout();
        let areas = untrack(|| self.areas());
        let (Some(from), Some(to)) = (
            untrack(|| self.display_of(tab)).and_then(|from| areas.get(from).copied()),
            areas.get(display).copied(),
        ) else {
            return;
        };
        let snap = untrack(|| layout.window_snap(&tab));
        if snap.is_some() {
            layout.unsnap_window(&tab);
        }
        let Some(rect) = untrack(|| layout.window_rect(&tab)) else {
            return;
        };
        layout.move_window(&tab, moved_onto(rect, from, to));
        if let Some(snap) = snap {
            layout.snap_window(&tab, snap, to);
        }
    }

    pub fn step_display(&self, tab: TabId, backwards: bool) {
        let count = untrack(|| self.areas()).len();
        let Some(own) = untrack(|| self.display_of(tab)) else {
            return;
        };
        if count < 2 {
            return;
        }
        let next = match backwards {
            true => (own + count - 1) % count,
            false => (own + 1) % count,
        };
        self.move_to_display(tab, next);
    }

    pub fn shows_desktop(&self) -> bool {
        self.cleared.with(Option::is_some) && self.clearable().is_empty()
    }

    pub fn show_desktop(&self) {
        let shown = untrack(|| self.clearable());
        if shown.is_empty()
            && let Some(cleared) = self.cleared.get_untracked()
        {
            self.cleared.set(None);
            for window in cleared.into_iter().rev() {
                match window {
                    Cleared::Window(id) => self.workspace.show(window_tab(id)),
                    Cleared::Block(tab) => self.workspace.show(tab),
                }
            }
            return;
        }
        let recent = untrack(|| self.workspace.layout().recent());
        let mut order: Vec<TabId> = recent
            .into_iter()
            .filter(|tab| shown.contains(tab))
            .collect();
        let rest: Vec<TabId> = shown
            .iter()
            .copied()
            .filter(|tab| !order.contains(tab))
            .collect();
        order.extend(rest);
        let mut cleared = Vec::new();
        for tab in order {
            match untrack(|| self.workspace.program_window(tab)) {
                Some(window) => cleared.push(Cleared::Window(window)),
                None => cleared.push(Cleared::Block(tab)),
            }
            self.workspace.hide(tab);
        }
        self.cleared.set(Some(cleared));
    }

    pub fn next_of_app(&self, tab: TabId, backwards: bool) {
        let Some(kind) = untrack(|| self.workspace.kind_of(tab)) else {
            return;
        };
        let mut same: Vec<TabId> = untrack(|| {
            self.workspace
                .shown_window_tabs()
                .into_iter()
                .filter(|other| self.workspace.kind_of(*other).as_ref() == Some(&kind))
                .collect()
        });
        same.sort();
        let Some(at) = same.iter().position(|other| *other == tab) else {
            return;
        };
        if same.len() < 2 {
            return;
        }
        let next = match backwards {
            true => same[(at + same.len() - 1) % same.len()],
            false => same[(at + 1) % same.len()],
        };
        self.workspace.layout().show(&next);
    }

    pub fn actions(&self) -> WindowActions {
        let manager = *self;
        let locked = self.locked;
        let focused = create_memo(move || match locked.get() {
            true => None,
            false => manager.focused(),
        });
        let snap = create_memo(move || {
            focused
                .get()
                .and_then(|tab| manager.workspace.layout().window_snap(&tab))
        });
        let has_focus = create_memo(move || focused.get().is_some());
        let snapped_as = move |wanted: DockSnap| {
            create_memo(move || focused.get().is_some() && snap.get() != Some(wanted))
        };
        let on_focused = move |act: fn(&WindowManager, TabId)| {
            move || {
                if let Some(tab) = focused.get_untracked() {
                    act(&manager, tab);
                }
            }
        };
        let several = create_memo(move || has_focus.get() && manager.areas().len() > 1);
        let can_clear = create_memo(move || {
            !locked.get()
                && (!manager.clearable().is_empty() || manager.shows_desktop())
        });
        let shows_desktop = create_memo(move || manager.shows_desktop());
        let more_of_app = create_memo(move || {
            let Some(tab) = focused.get() else {
                return false;
            };
            let kind = manager.workspace.kind_of(tab);
            kind.is_some()
                && manager
                    .workspace
                    .shown_window_tabs()
                    .into_iter()
                    .filter(|other| manager.workspace.kind_of(*other) == kind)
                    .count()
                    > 1
        });
        WindowActions {
            close: Action::new(
                "desktop.window.close",
                "Close window",
                on_focused(|manager, tab| manager.close(tab)),
            )
            .glyph(ICON_X)
            .enabled(has_focus),
            snap_left: Action::new(
                "desktop.window.snap_left",
                "Snap window to the left half",
                on_focused(|manager, tab| match manager.workspace.layout().window_snap(&tab) {
                    Some(DockSnap::Right) => {
                        manager.restore(tab);
                    }
                    _ => manager.snap(tab, DockSnap::Left),
                }),
            )
            .glyph(ICON_PANEL_LEFT)
            .enabled(snapped_as(DockSnap::Left)),
            snap_right: Action::new(
                "desktop.window.snap_right",
                "Snap window to the right half",
                on_focused(|manager, tab| match manager.workspace.layout().window_snap(&tab) {
                    Some(DockSnap::Left) => {
                        manager.restore(tab);
                    }
                    _ => manager.snap(tab, DockSnap::Right),
                }),
            )
            .glyph(ICON_PANEL_RIGHT)
            .enabled(snapped_as(DockSnap::Right)),
            maximize: Action::new(
                "desktop.window.maximize",
                "Maximize window",
                on_focused(|manager, tab| manager.snap(tab, DockSnap::Fill)),
            )
            .glyph(ICON_MAXIMIZE_2)
            .enabled(snapped_as(DockSnap::Fill)),
            restore: Action::new(
                "desktop.window.restore",
                "Restore window",
                on_focused(|manager, tab| {
                    manager.restore(tab);
                }),
            )
            .glyph(ICON_MINIMIZE_2)
            .enabled(create_memo(move || snap.get().is_some())),
            hide: Action::new(
                "desktop.window.hide",
                "Hide window",
                on_focused(|manager, tab| manager.hide(tab)),
            )
            .glyph(ICON_EYE_OFF)
            .enabled(create_memo(move || has_focus.get() && snap.get().is_none())),
            show_desktop: Action::new("desktop.window.show_desktop", "Show desktop", move || {
                manager.show_desktop()
            })
            .glyph(ICON_MONITOR)
            .checked(shows_desktop)
            .enabled(can_clear),
            next_display: Action::new(
                "desktop.window.next_display",
                "Move window to the next display",
                on_focused(|manager, tab| manager.step_display(tab, false)),
            )
            .glyph(ICON_ARROW_RIGHT_TO_LINE)
            .enabled(several),
            previous_display: Action::new(
                "desktop.window.previous_display",
                "Move window to the previous display",
                on_focused(|manager, tab| manager.step_display(tab, true)),
            )
            .glyph(ICON_ARROW_LEFT_TO_LINE)
            .enabled(several),
            next_of_app: Action::new(
                "desktop.window.next_of_app",
                "Switch to the app's next window",
                on_focused(|manager, tab| manager.next_of_app(tab, false)),
            )
            .glyph(ICON_LAYERS)
            .enabled(more_of_app),
        }
    }
}
