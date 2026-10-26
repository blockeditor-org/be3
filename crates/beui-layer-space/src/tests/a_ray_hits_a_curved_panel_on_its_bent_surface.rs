use super::*;

#[test]
fn a_ray_hits_a_curved_panel_on_its_bent_surface() {
    let (mut space, now) = space();
    let mut curved = panel(WindowId::MAIN, ahead(1.0));
    curved.bend = 1.5;
    let flat = panel(WindowId::MAIN, ahead(1.0));
    assert!(
        curved.world(pos2(0.0, 250.0)).z > curved.pose.position.z + 0.05,
        "its sides come toward the viewer"
    );
    assert!(close(
        curved.world(pos2(500.0, 250.0)),
        curved.pose.position
    ));

    let eye = Head::default().position;
    for pos in [pos2(40.0, 30.0), pos2(500.0, 250.0), pos2(930.0, 420.0)] {
        let toward = curved.world(pos);
        let ray = crate::head::Ray {
            origin: eye,
            direction: (toward - eye).normalize(),
        };
        let hit = curved.hit(ray).expect("the ray meets the curved panel");
        assert!(
            (hit.pos - pos).length() < 0.01,
            "{pos:?} picks {:?}",
            hit.pos
        );
        assert!(close(hit.point, toward));
        if pos.x < 100.0 || pos.x > 900.0 {
            let plane = flat.intersect(ray).unwrap().pos;
            assert!(
                (plane - pos).length() > 20.0,
                "a flat panel would put {pos:?} at {plane:?}"
            );
        }
    }

    space.panels[0] = Panel {
        size: PLATFORM,
        ..curved
    };
    let pos = pos2(80.0, 500.0);
    let at = screen(&space, main(&space).world(pos));
    let got = delivered(&send(&mut space, now, [Input::PointerMoved(at)]));
    let [(WindowId::MAIN, Input::PointerMoved(moved))] = got.as_slice() else {
        panic!("the app gets the move, not {got:?}");
    };
    assert!(
        (*moved - pos).length() < 0.5,
        "the app is told where on the curve the cursor is: {moved:?}"
    );
}
