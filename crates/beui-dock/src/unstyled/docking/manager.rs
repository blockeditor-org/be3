use beui_view::reactive::*;
use beui_view::types::icons::{
    ICON_ARROW_LEFT_TO_LINE, ICON_ARROW_RIGHT_TO_LINE, ICON_BRING_TO_FRONT, ICON_CIRCLE_X,
    ICON_EYE_OFF, ICON_LAYERS, ICON_MAXIMIZE, ICON_MAXIMIZE_2, ICON_MINIMIZE_2, ICON_MONITOR,
    ICON_PANEL_LEFT, ICON_PANEL_RIGHT, ICON_X,
};
use beui_view::types::{Pos2, Rect, Vec2};

use super::{DockKey, DockSnap, DockState, DockingLayout, TabId, standing_in};
use crate::unstyled::state::SurfaceId;

pub struct DockWindowActions {
    pub close: ActionBuilder,
    pub hide: ActionBuilder,
    pub snap_left: ActionBuilder,
    pub snap_right: ActionBuilder,
    pub maximize: ActionBuilder,
    pub restore: ActionBuilder,
    pub fullscreen: ActionBuilder,
    pub show_desktop: ActionBuilder,
    pub next_area: ActionBuilder,
    pub previous_area: ActionBuilder,
    pub next_of_group: ActionBuilder,
    pub switch_next: ActionBuilder,
    pub switch_previous: ActionBuilder,
    pub switch_cancel: ActionBuilder,
}

fn overlap(left: Rect, right: Rect) -> f32 {
    let shared = left.intersect(right);
    shared.width().max(0.0) * shared.height().max(0.0)
}

pub fn holding_area(window: Rect, areas: &[Rect]) -> Option<usize> {
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

fn floating_surface(state: &DockState, tab: TabId) -> Option<SurfaceId> {
    let surface = state.find(standing_in(state, tab))?.surface;
    state.window_rect(surface).map(|_| surface)
}

impl<K: DockKey> DockingLayout<K> {
    pub fn set_areas(&self, areas: Vec<Rect>) {
        if self.areas.with_untracked(|held| *held != areas) {
            self.areas.set(areas);
        }
    }

    pub fn areas(&self) -> Vec<Rect> {
        let areas = self.areas.get();
        match areas.is_empty() {
            true => vec![self.area()],
            false => areas,
        }
    }

    pub fn area_of_window(&self, key: &K) -> Option<usize> {
        let rect = self.window_rect(key)?;
        holding_area(rect, &self.areas())
    }

    fn window_area(&self, key: &K) -> Option<Rect> {
        let area = self.area_of_window(key)?;
        self.areas().get(area).copied()
    }

    pub fn move_to_area(&self, key: &K, area: usize) {
        let areas = untrack(|| self.areas());
        let (Some(from), Some(to)) = (
            untrack(|| self.area_of_window(key)).and_then(|from| areas.get(from).copied()),
            areas.get(area).copied(),
        ) else {
            return;
        };
        let snap = untrack(|| self.window_snap(key));
        if snap.is_some() {
            self.unsnap_window(key);
        }
        let Some(rect) = untrack(|| self.window_rect(key)) else {
            return;
        };
        self.move_window(key, moved_onto(rect, from, to));
        if let Some(snap) = snap {
            self.snap_window(key, snap, to);
        }
    }

    pub fn step_area(&self, key: &K, backwards: bool) {
        let count = untrack(|| self.areas()).len();
        let Some(own) = untrack(|| self.area_of_window(key)) else {
            return;
        };
        if count < 2 {
            return;
        }
        let next = match backwards {
            true => (own + count - 1) % count,
            false => (own + 1) % count,
        };
        self.move_to_area(key, next);
    }

    pub fn snap_in_area(&self, key: &K, snap: DockSnap) {
        if let Some(area) = untrack(|| self.window_area(key)) {
            self.snap_window(key, snap, area);
        }
    }

    pub fn is_hidden(&self, key: &K) -> bool {
        let Some(tab) = self.tab_of(key) else {
            return false;
        };
        self.state.with(|state| {
            floating_surface(state, tab).is_some_and(|surface| state.is_hidden(surface))
        })
    }

    pub fn hide_window(&self, key: &K) {
        let Some(tab) = untrack(|| self.tab_of(key)) else {
            return;
        };
        self.edit(|state| {
            if let Some(surface) = floating_surface(state, tab) {
                state.hide_window(surface);
            }
        });
    }

    pub fn shown_windows(&self) -> Vec<K> {
        let tabs = self.state.with(|state| {
            let mut tabs = Vec::new();
            for surface in state.shown_windows() {
                tabs.extend(state.surface_tabs(surface));
            }
            tabs
        });
        tabs.into_iter()
            .filter_map(|tab| self.key_of(tab))
            .collect()
    }

    fn cleared_windows(&self) -> Vec<TabId> {
        self.state.with(|state| {
            state
                .shown_windows()
                .into_iter()
                .filter_map(|surface| {
                    let leaves = state.leaves(surface);
                    leaves
                        .iter()
                        .find(|leaf| Some(**leaf) == state.focused_leaf())
                        .or(leaves.first())
                        .and_then(|leaf| state.active_tab(*leaf))
                })
                .collect()
        })
    }

    pub fn shows_desktop(&self) -> bool {
        self.cleared.with(Option::is_some) && self.cleared_windows().is_empty()
    }

    pub fn show_desktop(&self) {
        let shown = untrack(|| self.cleared_windows());
        if shown.is_empty()
            && let Some(cleared) = self.cleared.get_untracked()
        {
            self.cleared.set(None);
            self.edit(|state| {
                for tab in cleared.into_iter().rev() {
                    state.show(tab);
                }
            });
            return;
        }
        if shown.is_empty() {
            return;
        }
        let recent = untrack(|| self.state.with(DockState::recent_tabs));
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
        self.edit(|state| {
            for tab in &order {
                if let Some(surface) = floating_surface(state, *tab) {
                    state.hide_window(surface);
                }
            }
        });
        self.cleared.set(Some(order));
    }

    pub fn group_of(&self, key: &K) -> Option<String> {
        let tab = self.tab_of(key)?;
        let declared = self.inner().declared_by.get()?;
        let found = declared.found;
        found.is_alive().then(|| found.with(|found| found.group(tab)))?
    }

    fn group_mates(&self, key: &K) -> Vec<K> {
        let Some(group) = self.group_of(key) else {
            return Vec::new();
        };
        let mut same: Vec<(TabId, K)> = self
            .shown_windows()
            .into_iter()
            .filter(|other| self.group_of(other).as_ref() == Some(&group))
            .filter_map(|other| Some((self.tab_of(&other)?, other)))
            .collect();
        same.sort_by_key(|(tab, _)| *tab);
        same.into_iter().map(|(_, other)| other).collect()
    }

    pub fn next_of_group(&self, key: &K, backwards: bool) {
        let same = untrack(|| self.group_mates(key));
        let Some(at) = same.iter().position(|other| other == key) else {
            return;
        };
        if same.len() < 2 {
            return;
        }
        let next = match backwards {
            true => &same[(at + same.len() - 1) % same.len()],
            false => &same[(at + 1) % same.len()],
        };
        self.show(next);
    }

    pub fn close_tab(&self, key: &K) {
        let Some(tab) = untrack(|| self.tab_of(key)) else {
            return;
        };
        if let Some(declared) = self.inner().declared_by.get() {
            declared.close.call(tab);
        }
    }

    pub fn toggle_window_fullscreen(&self, key: &K) {
        match untrack(|| self.fullscreen()).as_ref() == Some(key) {
            true => self.leave_fullscreen(),
            false => {
                let area = untrack(|| self.window_area(key));
                self.enter_fullscreen(key, area);
            }
        }
    }

    fn dialog_root(&self, tab: TabId) -> TabId {
        let Some(declared) = self.inner().declared_by.get() else {
            return tab;
        };
        let mut tab = tab;
        for _ in 0..super::MAX_JOINED_DEPTH {
            let found = declared.found;
            match found
                .is_alive()
                .then(|| found.with(|found| found.dialog_parent(tab)))
                .flatten()
            {
                Some(parent) => tab = parent,
                None => break,
            }
        }
        tab
    }

    pub fn focused_window(&self) -> Option<K> {
        let focused = self.tab_of(&self.focused()?)?;
        let dialog = self
            .inner()
            .registry
            .dock
            .get()
            .and_then(|dock| dock.try_get())
            .and_then(|dock| dock.focused_dialog())
            .filter(|dialog| self.dialog_root(*dialog) == focused);
        let focused = dialog.unwrap_or(focused);
        let root = self.dialog_root(focused);
        let shown = self.state.with(|state| {
            floating_surface(state, root).is_some_and(|surface| !state.is_hidden(surface))
        });
        shown.then(|| self.key_of(focused)).flatten()
    }

    fn focused_root(&self) -> Option<K> {
        let focused = self.tab_of(&self.focused_window()?)?;
        self.key_of(self.dialog_root(focused))
    }

    pub fn window_actions(&self, disabled: Memo<bool>) -> DockWindowActions {
        let layout = *self;
        let focused = create_memo(move || match disabled.get() {
            true => None,
            false => layout.focused_window(),
        });
        let root = create_memo(move || {
            focused.get()?;
            layout.focused_root()
        });
        let snap = create_memo(move || root.get().and_then(|key| layout.window_snap(&key)));
        let has_focus = create_memo(move || focused.get().is_some());
        let snapped_as = move |wanted: DockSnap| {
            create_memo(move || root.get().is_some() && snap.get() != Some(wanted))
        };
        let on_root = move |act: fn(&DockingLayout<K>, &K)| {
            move || {
                if let Some(key) = root.get_untracked() {
                    act(&layout, &key);
                }
            }
        };
        let several = create_memo(move || root.get().is_some() && layout.areas().len() > 1);
        let can_clear = create_memo(move || {
            !disabled.get() && (!layout.cleared_windows().is_empty() || layout.shows_desktop())
        });
        let shows_desktop = create_memo(move || layout.shows_desktop());
        let more_of_group = create_memo(move || {
            root.get()
                .is_some_and(|key| layout.group_mates(&key).len() > 1)
        });
        let fullscreen = create_memo(move || {
            root.get()
                .is_some_and(|key| layout.fullscreen().as_ref() == Some(&key))
        });
        let switchable = create_memo(move || {
            !disabled.get() && layout.state.with(|state| state.all_tabs().len() > 1)
        });
        let switching = create_memo(move || !disabled.get() && layout.switching());
        let modifiers = held_modifiers();
        let holding = move |held: beui_view::types::Modifiers| held.alt || held.ctrl || held.logo;
        create_effect(move || {
            if !holding(modifiers.get()) && untrack(|| layout.switching()) {
                untrack(|| layout.commit_switch());
            }
        });
        let switch = move |backwards: bool| {
            move || {
                match layout.switching() {
                    true => layout.step_switch(backwards),
                    false => layout.begin_switch(backwards),
                }
                if !holding(modifiers.get_untracked()) {
                    layout.commit_switch();
                }
            }
        };
        DockWindowActions {
            close: Action::new(
                "dock.window.close",
                "Close window",
                move || {
                    if let Some(key) = focused.get_untracked() {
                        layout.close_tab(&key);
                    }
                },
            )
            .glyph(ICON_X)
            .enabled(has_focus),
            hide: Action::new(
                "dock.window.hide",
                "Hide window",
                on_root(DockingLayout::hide_window),
            )
            .glyph(ICON_EYE_OFF)
            .enabled(create_memo(move || {
                root.get().is_some() && snap.get().is_none()
            })),
            snap_left: Action::new(
                "dock.window.snap_left",
                "Snap window to the left half",
                on_root(|layout, key| match layout.window_snap(key) {
                    Some(DockSnap::Right) => {
                        layout.unsnap_window(key);
                    }
                    _ => layout.snap_in_area(key, DockSnap::Left),
                }),
            )
            .glyph(ICON_PANEL_LEFT)
            .enabled(snapped_as(DockSnap::Left)),
            snap_right: Action::new(
                "dock.window.snap_right",
                "Snap window to the right half",
                on_root(|layout, key| match layout.window_snap(key) {
                    Some(DockSnap::Left) => {
                        layout.unsnap_window(key);
                    }
                    _ => layout.snap_in_area(key, DockSnap::Right),
                }),
            )
            .glyph(ICON_PANEL_RIGHT)
            .enabled(snapped_as(DockSnap::Right)),
            maximize: Action::new(
                "dock.window.maximize",
                "Maximize window",
                on_root(|layout, key| layout.snap_in_area(key, DockSnap::Fill)),
            )
            .glyph(ICON_MAXIMIZE_2)
            .enabled(snapped_as(DockSnap::Fill)),
            restore: Action::new(
                "dock.window.restore",
                "Restore window",
                on_root(|layout, key| {
                    layout.unsnap_window(key);
                }),
            )
            .glyph(ICON_MINIMIZE_2)
            .enabled(create_memo(move || snap.get().is_some())),
            fullscreen: Action::new(
                "dock.window.fullscreen",
                "Show window full screen",
                on_root(DockingLayout::toggle_window_fullscreen),
            )
            .glyph(ICON_MAXIMIZE)
            .checked(fullscreen)
            .enabled(create_memo(move || root.get().is_some())),
            show_desktop: Action::new("dock.window.show_desktop", "Show desktop", move || {
                layout.show_desktop();
            })
            .glyph(ICON_MONITOR)
            .checked(shows_desktop)
            .enabled(can_clear),
            next_area: Action::new(
                "dock.window.next_area",
                "Move window to the next display",
                on_root(|layout, key| layout.step_area(key, false)),
            )
            .glyph(ICON_ARROW_RIGHT_TO_LINE)
            .enabled(several),
            previous_area: Action::new(
                "dock.window.previous_area",
                "Move window to the previous display",
                on_root(|layout, key| layout.step_area(key, true)),
            )
            .glyph(ICON_ARROW_LEFT_TO_LINE)
            .enabled(several),
            next_of_group: Action::new(
                "dock.window.next_of_group",
                "Switch to the app's next window",
                on_root(|layout, key| layout.next_of_group(key, false)),
            )
            .glyph(ICON_LAYERS)
            .enabled(more_of_group),
            switch_next: Action::new(
                "dock.switch.next",
                "Switch to the next window",
                switch(false),
            )
            .glyph(ICON_BRING_TO_FRONT)
            .enabled(switchable),
            switch_previous: Action::new(
                "dock.switch.previous",
                "Switch to the previous window",
                switch(true),
            )
            .glyph(ICON_BRING_TO_FRONT)
            .enabled(switchable),
            switch_cancel: Action::new(
                "dock.switch.cancel",
                "Stop switching windows",
                move || layout.cancel_switch(),
            )
            .glyph(ICON_CIRCLE_X)
            .enabled(switching),
        }
    }
}
