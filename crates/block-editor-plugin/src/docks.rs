use stable_hash::HashMap;

use block_plugin_api::{
    ChildId, DockChild, DockChildClose, DockChildHosted, DockChildPick, DockDeclaration,
    DockDeclare, DockHold, DockHosted, DockMessage, DockRequest, DockTabPath, DockUpdate,
    DockViewRequest, EditorInstanceId, ViewId, ViewRef,
};

use crate::EditorHost;
use crate::host::Pushed;

#[derive(Default)]
pub(crate) struct Docks {
    used: bool,
    declared: HashMap<ViewId, Option<DockDeclaration>>,
    sent: HashMap<ViewId, Option<DockDeclaration>>,
    requests: Vec<(ViewId, DockRequest)>,
    hosting: HashMap<ChildId, DockHold>,
    hosted_sent: HashMap<ChildId, DockHold>,
    closing: Vec<(ChildId, DockTabPath)>,
    picking: Vec<(ChildId, DockTabPath, String)>,
    hosted: HashMap<ViewId, DockHosted>,
    closed: Vec<(ViewId, DockTabPath)>,
    children: HashMap<ChildId, DockDeclaration>,
    child_requests: Vec<(ChildId, Vec<ChildId>, DockRequest)>,
}

impl EditorHost {
    pub fn declare_dock(&self, view: ViewId, declaration: Option<DockDeclaration>) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.declared.insert(view, declaration);
        drop(docks);
        self.changed_docks();
    }

    pub fn request_dock(&self, view: ViewId, request: DockRequest) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.requests.push((view, request));
        drop(docks);
        self.changed_docks();
    }

    pub fn dock_hosted(&self, view: ViewId) -> Option<DockHosted> {
        self.docks.borrow().hosted.get(&view).cloned()
    }

    pub fn take_dock_closes(&self, view: ViewId) -> Vec<DockTabPath> {
        let mut docks = self.docks.borrow_mut();
        let mut taken = Vec::new();
        docks.closed.retain(|(held, tab)| {
            let here = *held == view;
            if here {
                taken.push(tab.clone());
            }
            !here
        });
        taken
    }

    pub fn child_dock(&self, child: ChildId) -> Option<DockDeclaration> {
        self.docks.borrow().children.get(&child).cloned()
    }

    pub fn take_child_dock_requests(&self, child: ChildId) -> Vec<(Vec<ChildId>, DockRequest)> {
        let mut docks = self.docks.borrow_mut();
        let mut taken = Vec::new();
        docks.child_requests.retain(|(held, owner, request)| {
            let here = *held == child;
            if here {
                taken.push((owner.clone(), request.clone()));
            }
            !here
        });
        taken
    }

    pub fn host_child_dock(&self, child: ChildId, hold: DockHold) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        if docks.hosting.get(&child) == Some(&hold) {
            return;
        }
        docks.hosting.insert(child, hold);
        drop(docks);
        self.changed_docks();
    }

    pub fn close_child_dock_tab(&self, child: ChildId, tab: DockTabPath) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.closing.push((child, tab));
        drop(docks);
        self.changed_docks();
    }

    pub fn pick_child_dock_tab(&self, child: ChildId, tab: DockTabPath, id: String) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.picking.push((child, tab, id));
        drop(docks);
        self.changed_docks();
    }

    pub fn receive_dock_updates(&self, updates: Vec<DockUpdate>) {
        if updates.is_empty() {
            return;
        }
        let mut docks = self.docks.borrow_mut();
        for update in updates {
            match update {
                DockUpdate::Hosted(hosted) => match hosted.hosted {
                    Some(held) => {
                        docks.hosted.insert(hosted.view, held);
                    }
                    None => {
                        docks.hosted.remove(&hosted.view);
                    }
                },
                DockUpdate::Close(close) => docks.closed.push((close.view, close.tab)),
                DockUpdate::Child(child) => match child.declaration {
                    Some(declaration) => {
                        docks.children.insert(child.child, declaration);
                    }
                    None => {
                        docks.children.remove(&child.child);
                    }
                },
                DockUpdate::ChildRequest(request) => {
                    docks
                        .child_requests
                        .push((request.child, request.owner, request.request));
                }
            }
        }
        drop(docks);
        self.push(Pushed::Docks);
        self.waker().wake();
    }

    pub(crate) fn uses_docks(&self) -> bool {
        self.docks.borrow().used
    }

    pub(crate) fn hosting_children(&self) -> Vec<ChildId> {
        self.docks
            .borrow()
            .hosting
            .iter()
            .filter(|(_, hold)| !matches!(hold, DockHold::Released))
            .map(|(child, _)| *child)
            .collect()
    }

    pub(crate) fn mark_docks_used(&self) {
        self.docks.borrow_mut().used = true;
    }

    pub(crate) fn take_dock_messages(&self, instance: EditorInstanceId) -> Vec<DockMessage> {
        let mut docks = self.docks.borrow_mut();
        let docks = &mut *docks;
        let mut messages = Vec::new();
        let mut declared: Vec<_> = docks.declared.iter().collect();
        declared.sort_by_key(|(view, _)| **view);
        for (view, declaration) in declared {
            if docks.sent.get(view) == Some(declaration)
                || (declaration.is_none() && !docks.sent.contains_key(view))
            {
                continue;
            }
            docks.sent.insert(*view, declaration.clone());
            messages.push(DockMessage::Declare(DockDeclare {
                view: ViewRef {
                    instance,
                    view: *view,
                },
                declaration: declaration.clone(),
            }));
        }
        for (view, request) in docks.requests.drain(..) {
            messages.push(DockMessage::Request(DockViewRequest {
                view: ViewRef { instance, view },
                request,
            }));
        }
        let mut hosting: Vec<_> = docks.hosting.iter().collect();
        hosting.sort_by_key(|(child, _)| **child);
        for (child, hold) in hosting {
            if docks.hosted_sent.get(child) == Some(hold)
                || (!matches!(hold, DockHold::Hosted(_)) && !docks.hosted_sent.contains_key(child))
            {
                continue;
            }
            docks.hosted_sent.insert(*child, hold.clone());
            messages.push(DockMessage::Host(DockChildHosted {
                child: DockChild {
                    instance,
                    child: *child,
                },
                hold: hold.clone(),
            }));
        }
        for (child, tab) in docks.closing.drain(..) {
            messages.push(DockMessage::Close(DockChildClose {
                child: DockChild { instance, child },
                tab,
            }));
        }
        for (child, tab, id) in docks.picking.drain(..) {
            messages.push(DockMessage::Pick(DockChildPick {
                child: DockChild { instance, child },
                tab,
                id,
                anchor: None,
            }));
        }
        messages
    }

    fn changed_docks(&self) {
        self.request_frame_in(std::time::Duration::ZERO);
    }
}
