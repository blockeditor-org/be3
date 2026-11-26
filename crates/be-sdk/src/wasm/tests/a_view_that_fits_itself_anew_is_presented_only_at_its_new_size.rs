use super::*;

#[test]
fn a_view_that_fits_itself_anew_is_presented_only_at_its_new_size() {
    // A view that fits itself to its content (`ScreenRequest::fit`) and measures new content in
    // a frame is run at the placement it had; its surface is placed anew at the fitted size, so
    // that frame's painting is not presented into it, and the app owes the host the frame that
    // draws it at its new size, whatever host runs it.
    let mut rig = Rig::new(Some(space()));
    rig.requests[0].fit = true;
    *rig.shared.fitted.borrow_mut() = Some(Vec2 { x: 50.0, y: 40.0 });
    let (_, presented) = rig.draw_and_present();
    let surface = rig.screen_surface();
    let stale: Vec<_> = presented
        .presented
        .iter()
        .flat_map(|frame| &frame.painted)
        .filter(|placement| placement.surface == surface)
        .map(|placement| (placement.width, placement.height))
        .collect();
    assert_eq!(
        stale,
        [],
        "the frame that fitted the view presents nothing at its old size"
    );
    assert_eq!(
        (rig.placed(SCREEN).width, rig.placed(SCREEN).height),
        (100, 80),
        "the surface is placed at the fitted size, at the screen's raster"
    );
    assert!(
        rig.surfaces.take_owed(),
        "the app asks the host for the frame that draws it at its new size"
    );
    let (_, presented) = rig.draw_and_present();
    let drawn: Vec<_> = presented
        .presented
        .iter()
        .flat_map(|frame| &frame.painted)
        .filter(|placement| placement.surface == surface)
        .map(|placement| (placement.width, placement.height))
        .collect();
    assert_eq!(
        drawn,
        [(100, 80)],
        "the next frame presents it at its new size"
    );
    assert!(!rig.surfaces.take_owed(), "and owes no other");
}
