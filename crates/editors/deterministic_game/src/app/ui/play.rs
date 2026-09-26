use std::rc::Rc;

use block_editor_beui::beui::reactive::{Memo, ReadSignal, WriteSignal};
use game_api::{Board, Gesture, Spot};

use super::{Action, GameModel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mark {
    None,
    Movable,
    Selected,
    Target,
    Over,
}

#[derive(Clone)]
pub(crate) struct Play {
    pub(crate) game: Rc<dyn GameModel>,
    pub(crate) actions: Memo<Vec<Action>>,
    pub(crate) editable: Memo<bool>,
    pub(crate) board: Memo<Board>,
    pub(crate) selected: ReadSignal<Option<Spot>>,
    pub(crate) set_selected: WriteSignal<Option<Spot>>,
    pub(crate) choices: ReadSignal<Vec<Action>>,
    pub(crate) set_choices: WriteSignal<Vec<Action>>,
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
        self.actions.with(|actions| {
            let offers = |wanted: &dyn Fn(Gesture) -> bool| {
                actions
                    .iter()
                    .any(|action| action.gesture.is_some_and(wanted))
            };
            if offers(&|gesture| match gesture {
                Gesture::Click(clicked) => clicked == spot,
                Gesture::Drag { from, to } => Some(from) == selected && to == spot,
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

    pub(crate) fn movable(&self, spot: Spot) -> bool {
        self.editable.get()
            && self.actions.with(|actions| {
                actions.iter().any(|action| {
                    matches!(action.gesture, Some(Gesture::Drag { from, .. }) if from == spot)
                })
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
            self.set_selected.set(None);
            self.decide(clicked);
            return;
        }
        let pick = (selected != Some(spot) && self.movable_untracked(spot)).then_some(spot);
        self.set_choices.set(Vec::new());
        self.set_selected.set(pick);
    }

    pub(crate) fn dropped(&self, from: Spot, to: Spot) {
        if self.editable.get_untracked() {
            self.offer(Gesture::Drag { from, to });
        }
    }

    pub(crate) fn choose(&self, action: &Action) {
        self.set_selected.set(None);
        self.set_choices.set(Vec::new());
        self.game.choose(action.effect.clone());
    }

    pub(crate) fn cancel(&self) {
        self.set_selected.set(None);
        self.set_choices.set(Vec::new());
    }

    fn offer(&self, gesture: Gesture) {
        self.set_selected.set(None);
        self.decide(self.matching(gesture));
    }

    fn decide(&self, matching: Vec<Action>) {
        match matching.as_slice() {
            [] => self.set_choices.set(Vec::new()),
            [only] => self.choose(only),
            _ => self.set_choices.set(matching),
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

    fn movable_untracked(&self, spot: Spot) -> bool {
        self.actions.with_untracked(|actions| {
            actions.iter().any(
                |action| matches!(action.gesture, Some(Gesture::Drag { from, .. }) if from == spot),
            )
        })
    }
}
