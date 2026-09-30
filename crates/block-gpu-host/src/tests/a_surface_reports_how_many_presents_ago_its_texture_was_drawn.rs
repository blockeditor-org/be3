use super::*;

#[test]
fn a_surface_reports_how_many_presents_ago_its_texture_was_drawn() {
    let mut gpu = gpu();
    let mut ages = Vec::new();
    for width in [16, 16, 16, 16, 32, 32, 32] {
        gpu.configure_surface(0, &configuration(width, 8));
        gpu.acquire_surface(0);
        ages.push(gpu.surface_age(0));
        gpu.present_surface(0);
    }

    assert_eq!(
        ages,
        vec![0, 0, 2, 2, 0, 0, 2],
        "a texture holds nothing until it is presented, then what it held two presents ago, and a new size starts over"
    );
    assert_eq!(gpu.take_error(), None);
}
