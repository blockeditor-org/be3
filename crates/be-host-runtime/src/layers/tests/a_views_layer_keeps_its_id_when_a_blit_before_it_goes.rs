use super::*;

#[test]
fn a_views_layer_keeps_its_id_when_a_blit_before_it_goes() {
    let shared = Rc::new(RefCell::new(Shared::default()));
    shared.borrow_mut().paint(&[placement(1), placement(2)]);
    shared.borrow_mut().publish(
        &ScreenLayout {
            screens: vec![placement(1), placement(2)],
        },
        Vec::new(),
        None,
    );
    let editor = blit(
        &shared,
        1,
        Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0)),
    );
    let pane = blit(
        &shared,
        2,
        Rect::from_min_size(pos2(300.0, 0.0), vec2(200.0, 100.0)),
    );
    let id_of = |blits: &[(u64, Blit)]| {
        frame(blits, vec2(800.0, 600.0))
            .layers
            .iter()
            .find(|layer| {
                layer.surface
                    == Surface::Screen {
                        instance: 7,
                        surface: 2,
                    }
            })
            .map(|layer| layer.id)
    };
    let both = id_of(&[(1, editor), (2, pane.clone())]);
    assert!(both.is_some());
    assert_eq!(
        id_of(&[(2, pane.clone())]),
        both,
        "a layer's id counts only its own view's pieces"
    );
    assert_ne!(
        id_of(&[(3, pane)]),
        both,
        "and names the drawing that shows it, so another drawing of the view has its own"
    );
    let mut pieces = Pieces::default();
    assert_eq!(
        [
            pieces.next(7, 1, false, 1),
            pieces.next(7, 2, false, 1),
            pieces.next(7, 1, false, 1),
            pieces.next(7, 1, true, 1),
            pieces.next(7, 1, false, 2),
        ],
        [0, 0, 1, 0, 0]
    );
}
