use crate::document::Document;
use crate::node::NodeId;

#[derive(Default)]
pub struct FocusScopes(Vec<FocusScope>);

struct FocusScope {
    holder: NodeId,
    returns_to: Option<NodeId>,
    layer: Option<bool>,
}

impl Document {
    pub fn push_focus_scope(&mut self, holder: NodeId) {
        self.push_scope(holder, None);
    }

    pub fn pop_focus_scope(&mut self, holder: NodeId) {
        self.pop_scope(holder);
    }

    pub(crate) fn push_layer_scope(&mut self, holder: NodeId, traps: bool) {
        self.push_scope(holder, Some(traps));
    }

    fn push_scope(&mut self, holder: NodeId, layer: Option<bool>) {
        let returns_to = match self.focused {
            Some(focused) if self.is_within(focused, holder) => self
                .focus_before
                .filter(|before| !self.is_within(*before, holder)),
            focused => focused,
        };
        self.focus_scopes.0.retain(|scope| scope.holder != holder);
        self.focus_scopes.0.push(FocusScope {
            holder,
            returns_to,
            layer,
        });
    }

    pub(crate) fn pop_scope(&mut self, holder: NodeId) {
        let Some(at) = self
            .focus_scopes
            .0
            .iter()
            .position(|scope| scope.holder == holder)
        else {
            return;
        };
        let scope = self.focus_scopes.0.remove(at);
        let takes_back = match self.focused {
            Some(focused) => self.is_within(focused, holder),
            None => scope.layer == Some(true),
        };
        if !takes_back {
            return;
        }
        let target = self
            .focus_wanted
            .take()
            .into_iter()
            .chain(scope.returns_to)
            .find(|target| self.contains(*target) && self.focus_valid(*target));
        match (target, scope.layer) {
            (Some(target), _) => self.update_focus(Some(target)),
            (None, Some(_)) if self.focused.is_some() => self.update_focus(None),
            (None, _) => {}
        }
    }

    pub(crate) fn forget_focus_scope(&mut self, holder: NodeId) {
        self.focus_scopes.0.retain(|scope| scope.holder != holder);
    }

    pub(crate) fn note_focus_outside_scopes(&mut self, focus: NodeId) {
        if self.root.is_none_or(|root| !self.is_within(focus, root)) {
            return;
        }
        let outside: Vec<bool> = self
            .focus_scopes
            .0
            .iter()
            .map(|scope| !self.is_within(focus, scope.holder))
            .collect();
        for (scope, outside) in self.focus_scopes.0.iter_mut().zip(outside) {
            if outside {
                scope.returns_to = Some(focus);
            }
        }
    }

    pub(crate) fn returns_on_removal(&self, removed: NodeId) -> Option<NodeId> {
        self.focus_scopes
            .0
            .iter()
            .filter(|scope| self.contains(scope.holder) && self.is_within(scope.holder, removed))
            .filter(|scope| match self.focused {
                Some(focused) => self.is_within(focused, scope.holder),
                None => scope.layer == Some(true),
            })
            .find_map(|scope| {
                scope
                    .returns_to
                    .filter(|target| !self.is_within(*target, removed))
            })
    }

    pub(crate) fn return_removed_focus(&mut self, target: NodeId) {
        if self.focused.is_none() && self.contains(target) && self.focus_valid(target) {
            self.update_focus(Some(target));
        }
    }
}
