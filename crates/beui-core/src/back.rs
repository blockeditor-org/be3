use crate::document::Document;
use beui_tree::callback::Callback;
use beui_types::BackGesture;
use beui_types::NodeId;
use beui_types::NodeRef;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BackHandlerId(u64);

pub(crate) struct BackHandler {
    pub(crate) id: BackHandlerId,
    at: Option<NodeRef>,
    pub(crate) enabled: bool,
    pub(crate) on_back: Callback<BackGesture>,
}

#[derive(Default)]
pub struct BackHandlers {
    pub(crate) handlers: Vec<BackHandler>,
    next: u64,
}

impl BackHandlers {
    pub(crate) fn get(&self, id: BackHandlerId) -> Option<&BackHandler> {
        self.handlers.iter().find(|handler| handler.id == id)
    }
}

impl BackHandler {
    pub(crate) fn node(&self, root: Option<NodeId>) -> Option<NodeId> {
        match self.at {
            Some(at) => at.try_get(),
            None => root,
        }
    }
}

impl Document {
    pub fn add_back_handler(
        &mut self,
        at: Option<NodeRef>,
        on_back: Callback<BackGesture>,
    ) -> BackHandlerId {
        let handlers = &mut self.back_handlers;
        let id = BackHandlerId(handlers.next);
        handlers.next += 1;
        handlers.handlers.push(BackHandler {
            id,
            at,
            enabled: true,
            on_back,
        });
        id
    }

    pub fn set_back_handler_enabled(&mut self, id: BackHandlerId, enabled: bool) {
        let Some(handler) = self
            .back_handlers
            .handlers
            .iter_mut()
            .find(|handler| handler.id == id)
        else {
            return;
        };
        if handler.enabled == enabled {
            return;
        }
        handler.enabled = enabled;
        if !enabled {
            self.forget_back_handler_gesture(id);
        }
    }

    pub fn remove_back_handler(&mut self, id: BackHandlerId) {
        self.back_handlers
            .handlers
            .retain(|handler| handler.id != id);
        self.forget_back_handler_gesture(id);
    }
}
