use super::*;

fn frame(presents: u64, rect: SurfaceRect) -> Frame {
    Frame {
        textures: Vec::new(),
        presents,
        damage: Some(vec![ScreenDamage {
            screen: ScreenId(1),
            rect,
        }]),
    }
}

#[test]
fn a_frame_published_after_its_damage_was_read_is_redrawn_next() {
    let layout = ScreenLayout {
        screens: vec![ScreenPlacement {
            screen: ScreenId(1),
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            surface: 1,
            width: 200,
            height: 100,
            scale_factor_millis: 3000,
            raster_millis: 3000,
        }],
    };
    let whole = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    let pieces = [Piece {
        local: whole,
        source: whole,
    }];
    let shared = Rc::new(RefCell::new(Shared::default()));
    let blit = Blit {
        surface: 1,
        status: PresenterStatus::waiting(),
        shared: Rc::clone(&shared),
        screen: ScreenId(1),
        quad: Quad::upright(whole),
        source: whole,
        drawn: None,
        requested: None,
        placed: None,
    };
    let left = SurfaceRect {
        x: 0,
        y: 0,
        width: 20,
        height: 10,
    };
    let right = SurfaceRect {
        x: 180,
        y: 90,
        width: 20,
        height: 10,
    };

    shared.borrow_mut().publish(&layout, Some(frame(1, left)));
    assert_eq!(
        blit.damage(&pieces, 0),
        Some(vec![Rect::from_min_max(Pos2::ZERO, pos2(0.1, 0.1))]),
    );
    let seen = shared.borrow().published.count();
    shared.borrow_mut().publish(&layout, Some(frame(2, right)));
    assert_eq!(
        shared.borrow_mut().take_frames().len(),
        2,
        "the frame published after the damage was read is shown with the one before it",
    );

    assert_eq!(
        blit.damage(&pieces, seen),
        Some(vec![Rect::from_min_max(pos2(0.9, 0.9), pos2(1.0, 1.0))]),
        "the shown frame's damage is redrawn although it was taken before it was read",
    );
    assert_eq!(
        blit.damage(&pieces, shared.borrow().published.count()),
        Some(Vec::new()),
        "nothing published since is nothing to redraw",
    );
}
