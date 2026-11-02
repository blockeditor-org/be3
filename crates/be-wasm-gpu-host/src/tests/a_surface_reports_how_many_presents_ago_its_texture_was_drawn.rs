use super::*;

#[test]
fn a_surface_reports_how_many_presents_ago_its_texture_was_drawn() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    let mut ages = Vec::new();
    for width in [16, 16, 16, 16, 32, 32, 32] {
        gpu.surface_configure(surface, configuration(width, 8));
        gpu.surface_acquire(surface);
        ages.push(gpu.surface_age(surface));
        gpu.surface_present(surface);
    }

    assert_eq!(
        ages,
        vec![0, 0, 2, 2, 0, 0, 2],
        "a texture holds nothing until it is presented, then what it held two presents ago, and a new size starts over"
    );
    assert_eq!(gpu.take_error(), None);
}
