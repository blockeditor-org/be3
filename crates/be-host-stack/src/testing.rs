use std::sync::OnceLock;

use uuid::Uuid;

use crate::types::Source;

pub struct InProcess {
    pub sources: fn() -> Vec<Source>,
    pub knows: fn(Uuid) -> bool,
}

static IN_PROCESS: OnceLock<InProcess> = OnceLock::new();

pub fn serve_in_process(types: InProcess) {
    let _ = IN_PROCESS.set(types);
}

pub(crate) fn in_process() -> Option<&'static InProcess> {
    IN_PROCESS.get()
}
