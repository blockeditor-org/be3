use super::*;
use crate::{Boundary, Pending, Ready, hold};

#[test]
fn a_boundary_waits_for_what_its_scope_holds() {
    let _root = RootScope::new();
    let first = create_signal::<Ready<String>>(Err(Pending::Loading));
    let second = create_signal::<Ready<u32>>(Err(Pending::Loading));
    let scope = Scope::new();
    let (boundary, name, count) = scope.run(|| {
        let boundary = Boundary::provide();
        (boundary, hold(first), hold(second))
    });

    assert_eq!(boundary.state(), Err(Pending::Loading));
    assert_eq!(name.get(), "");

    first.set(Ok("notes".to_owned()));
    assert_eq!(boundary.state(), Err(Pending::Loading));
    assert_eq!(name.get(), "notes");

    second.set(Ok(3));
    assert_eq!(boundary.state(), Ok(()));
    assert_eq!(count.get(), 3);
}
