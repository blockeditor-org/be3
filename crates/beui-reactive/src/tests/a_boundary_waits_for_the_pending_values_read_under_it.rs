use super::*;
use crate::{Boundary, Pending, create_binding, create_source, hold};

#[test]
fn a_boundary_waits_for_the_pending_values_read_under_it() {
    let _root = RootScope::new();
    let first = create_source::<String>();
    let second = create_source::<u32>();
    let unread = create_source::<u32>();
    let scope = Scope::new();
    let (boundary, name) = scope.run(|| {
        let boundary = Boundary::provide();
        let name = hold(first);
        // A binding reads the held name, and the body reads the count once.
        create_binding(move || {
            name.track();
        });
        let _ = second.get_untracked();
        (boundary, name)
    });
    let _ = unread;

    assert_eq!(boundary.state(), Err(Pending::Loading));
    assert_eq!(name.get(), "");

    first.set(Ok("notes".to_owned()));
    assert_eq!(boundary.state(), Err(Pending::Loading));
    assert_eq!(name.get(), "notes");

    second.set(Ok(3));
    assert_eq!(boundary.state(), Ok(()), "a source nothing read does not hold it");
}
