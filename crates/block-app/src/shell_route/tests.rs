use std::collections::HashMap;

use super::*;

mod a_request_goes_to_the_nearest_editor_around_it_that_takes_it;
mod a_request_nothing_around_it_takes_goes_to_the_shell;

struct Nesting {
    shell: Uuid,
    parents: HashMap<Uuid, Uuid>,
    accepting: Vec<Uuid>,
}

impl Nesting {
    fn route(&self, from: Option<Uuid>) -> Uuid {
        handler(
            from,
            self.shell,
            |id| self.accepting.contains(&id),
            |id| self.parents.get(&id).copied(),
        )
    }
}
