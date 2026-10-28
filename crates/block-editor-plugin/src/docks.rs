use std::collections::HashMap;

use block_plugin_api::{
    ChildId, DockChild, DockChildClose, DockChildHosted, DockChildPick, DockDeclaration,
    DockDeclare, DockHosted, DockMessage, DockRegionRequest, DockRequest, DockTabPath, DockUpdate,
    EditorInstanceId, EditorRegion, RegionRef,
};

use crate::EditorHost;
use crate::host::Pushed;

#[derive(Default)]
pub(crate) struct Docks {
    used: bool,
    declared: HashMap<EditorRegion, Option<DockDeclaration>>,
    sent: HashMap<EditorRegion, Option<DockDeclaration>>,
    requests: Vec<(EditorRegion, DockRequest)>,
    hosting: HashMap<ChildId, Option<DockHosted>>,
    hosted_sent: HashMap<ChildId, Option<DockHosted>>,
    closing: Vec<(ChildId, DockTabPath)>,
    picking: Vec<(ChildId, DockTabPath, String)>,
    hosted: HashMap<EditorRegion, DockHosted>,
    closed: Vec<(EditorRegion, DockTabPath)>,
    picked: Vec<(EditorRegion, DockTabPath, String)>,
    children: HashMap<ChildId, DockDeclaration>,
    child_requests: Vec<(ChildId, Vec<ChildId>, DockRequest)>,
}

impl EditorHost {
    pub fn declare_dock(&self, region: EditorRegion, declaration: Option<DockDeclaration>) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.declared.insert(region, declaration);
        drop(docks);
        self.changed_docks();
    }

    pub fn request_dock(&self, region: EditorRegion, request: DockRequest) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        docks.requests.push((region, request));
        drop(docks);
        self.changed_docks();
    }

    pub fn dock_hosted(&self, region: EditorRegion) -> Option<DockHosted> {
        self.docks.borrow().hosted.get(&region).cloned()
    }

    pub fn take_dock_closes(&self, region: EditorRegion) -> Vec<DockTabPath> {
        let mut docks = self.docks.borrow_mut();
        let mut taken = Vec::new();
        docks.closed.retain(|(held, tab)| {
            let here = *held == region;
            if here {
                taken.push(tab.clone());
            }
            !here
        });
        taken
    }

    pub fn take_dock_picks(&self, region: EditorRegion) -> Vec<(DockTabPath, String)> {
        let mut docks = self.docks.borrow_mut();
        let mut taken = Vec::new();
        docks.picked.retain(|(held, tab, id)| {
            let here = *held == region;
            if here {
                taken.push((tab.clone(), id.clone()));
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

    pub fn host_child_dock(&self, child: ChildId, hosted: Option<DockHosted>) {
        let mut docks = self.docks.borrow_mut();
        docks.used = true;
        if docks.hosting.get(&child) == Some(&hosted) {
            return;
        }
        docks.hosting.insert(child, hosted);
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
                        docks.hosted.insert(hosted.region, held);
                    }
                    None => {
                        docks.hosted.remove(&hosted.region);
                    }
                },
                DockUpdate::Close(close) => docks.closed.push((close.region, close.tab)),
                DockUpdate::Pick(pick) => docks.picked.push((pick.region, pick.tab, pick.id)),
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

    pub(crate) fn mark_docks_used(&self) {
        self.docks.borrow_mut().used = true;
    }

    pub(crate) fn take_dock_messages(&self, instance: EditorInstanceId) -> Vec<DockMessage> {
        let mut docks = self.docks.borrow_mut();
        let docks = &mut *docks;
        let mut messages = Vec::new();
        let mut declared: Vec<_> = docks.declared.iter().collect();
        declared.sort_by_key(|(region, _)| format!("{region:?}"));
        for (region, declaration) in declared {
            if docks.sent.get(region) == Some(declaration)
                || (declaration.is_none() && !docks.sent.contains_key(region))
            {
                continue;
            }
            docks.sent.insert(*region, declaration.clone());
            messages.push(DockMessage::Declare(DockDeclare {
                region: RegionRef {
                    instance,
                    region: *region,
                },
                declaration: declaration.clone(),
            }));
        }
        for (region, request) in docks.requests.drain(..) {
            messages.push(DockMessage::Request(DockRegionRequest {
                region: RegionRef { instance, region },
                request,
            }));
        }
        let mut hosting: Vec<_> = docks.hosting.iter().collect();
        hosting.sort_by_key(|(child, _)| **child);
        for (child, hosted) in hosting {
            if docks.hosted_sent.get(child) == Some(hosted)
                || (hosted.is_none() && !docks.hosted_sent.contains_key(child))
            {
                continue;
            }
            docks.hosted_sent.insert(*child, hosted.clone());
            messages.push(DockMessage::Host(DockChildHosted {
                child: DockChild {
                    instance,
                    child: *child,
                },
                hosted: hosted.clone(),
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
            }));
        }
        messages
    }

    fn changed_docks(&self) {
        self.request_frame_in(std::time::Duration::ZERO);
    }
}
