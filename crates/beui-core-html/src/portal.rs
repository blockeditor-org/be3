use stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use beui_tree::reactive::{SlotHost, SlotId};

use crate::node::NodeId;

thread_local! {
    static HOLDERS: RefCell<HashMap<NodeId, Weak<Portal>>> = RefCell::default();
}

pub struct Portal {
    host: Rc<dyn SlotHost<NodeId>>,
    slot: SlotId,
    held: RefCell<Option<NodeId>>,
}

pub fn portal(node: NodeId) -> Rc<Portal> {
    let host = crate::tree::node_host(node);
    let slot = host.open();
    Rc::new(Portal {
        host,
        slot,
        held: RefCell::default(),
    })
}

impl Portal {
    pub fn hold(self: &Rc<Self>, child: Option<NodeId>) {
        if *self.held.borrow() == child {
            return;
        }
        self.release();
        let Some(child) = child else {
            return;
        };
        let shown = HOLDERS.with(|holders| {
            holders
                .borrow_mut()
                .insert(child, Rc::downgrade(self))
                .and_then(|held| held.upgrade())
        });
        if let Some(shown) = shown
            && !Rc::ptr_eq(&shown, self)
        {
            shown.held.replace(None);
            shown.host.fill(shown.slot, Vec::new());
        }
        *self.held.borrow_mut() = Some(child);
        self.host.fill(self.slot, vec![child]);
    }

    pub fn release(self: &Rc<Self>) {
        let Some(held) = self.held.take() else {
            return;
        };
        HOLDERS.with(|holders| {
            let mut holders = holders.borrow_mut();
            if holders
                .get(&held)
                .is_some_and(|holder| std::ptr::eq(holder.as_ptr(), Rc::as_ptr(self)))
            {
                holders.remove(&held);
            }
        });
        self.host.fill(self.slot, Vec::new());
    }
}
