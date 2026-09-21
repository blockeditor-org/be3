use uuid::Uuid;

use super::{Content, Status};

pub(crate) fn stop() {}

pub(crate) fn flush() {}

pub(crate) fn open(block: Uuid, content_type: Uuid) {
    let _ = (block, content_type);
}

pub(crate) fn close(block: Uuid) {
    let _ = block;
}

pub(crate) fn content(block: Uuid) -> Option<Content> {
    let _ = block;
    None
}

pub(crate) fn operate(block: Uuid, operation: Vec<u8>) {
    let _ = (block, operation);
}

pub(crate) fn status() -> Status {
    Status::default()
}
