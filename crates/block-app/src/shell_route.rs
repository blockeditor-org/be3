use std::collections::HashSet;

use uuid::Uuid;

pub(crate) fn handler(
    from: Option<Uuid>,
    shell: Uuid,
    accepts: impl Fn(Uuid) -> bool,
    parent: impl Fn(Uuid) -> Option<Uuid>,
) -> Uuid {
    let mut visited = HashSet::new();
    let mut current = from;
    while let Some(id) = current {
        if id == shell || !visited.insert(id) {
            break;
        }
        if accepts(id) {
            return id;
        }
        current = parent(id);
    }
    shell
}

#[cfg(test)]
mod tests;
