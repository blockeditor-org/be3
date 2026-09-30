use super::*;

#[test]
fn a_surface_reports_how_many_presents_ago_its_texture_was_drawn() {
    let mut direct = gpu();
    let mut recorder = Recorder::new(&direct.limits()).unwrap();
    let mut ages = Vec::new();
    let mut recorded = Vec::new();
    let mut frame = |width: u32, direct: &mut Gpu, recorder: &mut Recorder| {
        direct.configure_surface(0, &configuration(width, 8));
        recorder.configure_surface(0, &configuration(width, 8));
        direct.acquire_surface(0);
        recorder.acquire_surface(0);
        ages.push(direct.surface_age(0));
        recorded.push(recorder.surface_age(0));
        direct.present_surface(0);
        recorder.present_surface(0);
    };
    for width in [16, 16, 16, 16, 32, 32, 32] {
        frame(width, &mut direct, &mut recorder);
    }

    assert_eq!(
        ages,
        vec![0, 0, 2, 2, 0, 0, 2],
        "a texture holds nothing until it is presented, then what it held two presents ago, and a new size starts over"
    );
    assert_eq!(recorded, ages, "a recorder works out the same ages");
    assert_eq!(direct.take_error(), None);
}
