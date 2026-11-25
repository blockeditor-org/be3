use super::*;

#[test]
fn a_resource_of_another_package_is_one_handle() {
    let mut caller = Caller::new();
    let shape = embed::shapes::glue::make(&mut caller, 5);
    assert_eq!(embed::frames::glue::show(&mut caller, shape), 5);
    assert_eq!(embed::shapes::glue::shape_area(&mut caller, shape), 5);
    assert_eq!(caller.host.shown, [5]);
}
