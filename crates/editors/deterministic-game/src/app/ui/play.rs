use std::rc::Rc;

use block_editor_beui::beui::Pos2;
use block_editor_beui::beui::reactive::*;
use game_api::{Gesture, Spot};

use super::{Action, GameModel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mark {
    None,
    Movable,
    Selected,
    Target,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dragging {
    pub(crate) from: Spot,
    pub(crate) at: Pos2,
}

#[derive(Clone, Copy)]
pub(crate) struct Play {
    pub(crate) game: Stored<Rc<dyn GameModel>>,
    pub(crate) actions: Memo<Vec<Action>>,
    pub(crate) editable: Memo<bool>,
    pub(crate) selected: Signal<Option<Spot>>,
    pub(crate) choices: Signal<Vec<Action>>,
    pub(crate) dragging: Signal<Option<Dragging>>,
    pub(crate) over: Signal<Option<Spot>>,
}

impl Play {
    pub(crate) fn mark(&self, spot: Spot) -> Mark {
        if !self.editable.get() {
            return Mark::None;
        }
        let selected = self.selected.get();
        if selected == Some(spot) {
            return Mark::Selected;
        }
        if self.over.get() == Some(spot) && selected.is_some_and(|from| self.leads(from, spot)) {
            return Mark::Over;
        }
        self.actions.with(|actions| {
            let offers = |wanted: &dyn Fn(Gesture) -> bool| {
                actions
                    .iter()
                    .any(|action| action.gesture.is_some_and(wanted))
            };
            if offers(&|gesture| match gesture {
                Gesture::Click(clicked) => clicked == spot,
                Gesture::Drag { from, to } => Some(from) == selected && to == spot,
                Gesture::Control(_) => false,
            }) {
                Mark::Target
            } else if offers(
                &|gesture| matches!(gesture, Gesture::Drag { from, .. } if from == spot),
            ) {
                Mark::Movable
            } else {
                Mark::None
            }
        })
    }

    pub(crate) fn leads(&self, from: Spot, to: Spot) -> bool {
        self.actions.with_untracked(|actions| {
            actions
                .iter()
                .any(|action| action.gesture == Some(Gesture::Drag { from, to }))
        })
    }

    pub(crate) fn click(&self, spot: Spot) {
        if !self.editable.get_untracked() {
            return;
        }
        let selected = self.selected.get_untracked();
        if let Some(from) = selected
            && from != spot
            && self.leads(from, spot)
        {
            self.offer(Gesture::Drag { from, to: spot });
            return;
        }
        let clicked = self.matching(Gesture::Click(spot));
        if !clicked.is_empty() {
            self.selected.set(None);
            self.decide(clicked);
            return;
        }
        let pick = (selected != Some(spot) && self.movable_untracked(spot)).then_some(spot);
        self.choices.set(Vec::new());
        self.selected.set(pick);
    }

    pub(crate) fn dropped(&self, from: Spot, to: Spot) {
        if self.editable.get_untracked() {
            self.offer(Gesture::Drag { from, to });
        }
    }

    pub(crate) fn choose(&self, action: &Action) {
        self.selected.set(None);
        self.choices.set(Vec::new());
        self.game.value().choose(action.effect.clone());
    }

    pub(crate) fn cancel(&self) {
        self.selected.set(None);
        self.choices.set(Vec::new());
    }

    fn offer(&self, gesture: Gesture) {
        self.selected.set(None);
        self.decide(self.matching(gesture));
    }

    fn decide(&self, matching: Vec<Action>) {
        match matching.as_slice() {
            [] => self.choices.set(Vec::new()),
            [only] => self.choose(only),
            _ => self.choices.set(matching),
        }
    }

    fn matching(&self, gesture: Gesture) -> Vec<Action> {
        self.actions.with_untracked(|actions| {
            actions
                .iter()
                .filter(|action| action.gesture == Some(gesture))
                .cloned()
                .collect()
        })
    }

    pub(crate) fn can_drag(&self, spot: Spot) -> bool {
        self.editable.get_untracked() && self.movable_untracked(spot)
    }

    fn movable_untracked(&self, spot: Spot) -> bool {
        self.actions.with_untracked(|actions| {
            actions.iter().any(
                |action| matches!(action.gesture, Some(Gesture::Drag { from, .. }) if from == spot),
            )
        })
    }
}
