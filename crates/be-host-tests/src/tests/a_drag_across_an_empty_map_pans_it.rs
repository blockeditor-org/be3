use super::*;

const TILE: &str = "#map.underlay.2.0.0";

#[test]
fn a_drag_across_an_empty_map_pans_it() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Map");
    host.wait(TILE);
    host.gone("#editor.loading");
    host.run(&["settle"]);
    let stage = host.place("#map.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    let before = host.place(TILE);

    host.run(&["drag", &at(0.4, 0.5), &at(0.6, 0.6), "8"]);
    let after = host.place(TILE);
    let (dx, dy) = (after.0 - before.0, after.1 - before.1);
    assert!(
        (dx - stage.2 * 0.2).abs() < 3.0 && (dy - stage.3 * 0.1).abs() < 3.0,
        "the map moved with the pointer, {before:?} to {after:?} for a drag on {stage:?}"
    );
    host.close();
}
