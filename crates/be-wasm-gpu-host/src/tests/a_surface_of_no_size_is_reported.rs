use super::*;

#[test]
fn a_surface_of_no_size_is_reported() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    gpu.surface_configure(surface, configuration(0, 200));
    assert!(gpu.take_error().is_some());
    assert!(gpu.surface(0).is_none());
}
