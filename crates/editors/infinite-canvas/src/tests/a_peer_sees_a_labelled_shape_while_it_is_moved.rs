use super::*;

#[test]
fn a_peer_sees_a_labelled_shape_while_it_is_moved() {
    let mut shape = card();
    shape.text = "Plan".into();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    mover.presence_visible(true);
    mover.run();
    let mut held = BTreeMap::new();
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let label = format!("infinite-canvas.text.{}", shape.id);
    let (resting, written) = (watcher.rect_of(&item), watcher.rect_of(&label));

    let from = mover.rect_of(&item).center();
    let by = Vec2::new(120.0, 60.0);
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.next_frame();
    mover.pen(PenPhase::Move, from + by / 2.0, 0.5);
    mover.next_frame();
    mover.pen(PenPhase::Move, from + by, 0.5);
    mover.run();
    forward_presence(&mut mover, &mut watcher, &mut held);
    let drafted: Vec<CanvasDraft> = held
        .iter()
        .filter(|(kind, _)| CanvasDraft::holds(**kind))
        .map(|(_, value)| serde_json::from_slice(value).unwrap())
        .collect();
    let [CanvasDraft::Change { entity: moving, .. }] = &drafted[..] else {
        panic!("a shape being moved is drafted as a change to it, drafted {drafted:?}");
    };
    assert_eq!(moving.id, shape.id);
    assert_eq!(
        watcher.rect_of(&item).center(),
        resting.center() + by,
        "the peer draws the shape where it is being moved"
    );
    assert_eq!(
        watcher.rect_of(&label).center(),
        written.center() + by,
        "its label goes with it"
    );
    watcher.snapshot("a_peer_sees_a_labelled_shape_while_it_is_moved");

    mover.pen(PenPhase::Up(PointerButton::Primary), from + by, 0.0);
    mover.run();
    forward_presence(&mut mover, &mut watcher, &mut held);
    let moved = entities(&mover);
    assert_eq!(
        moved[0].transform.center,
        CanvasPoint::new(by.x, by.y),
        "the move is written"
    );
    assert_eq!(
        watcher.rect_of(&item).center(),
        resting.center() + by,
        "a peer the edit has not reached yet keeps drawing the shape where it was dropped"
    );

    apply(
        &mut watcher,
        InfiniteCanvasOperation::Update {
            entities: moved.clone(),
        },
    );
    watcher.run();
    assert_eq!(
        watcher.rect_of(&item).center(),
        resting.center() + by,
        "once the edit arrives the peer draws the stored shape in the same place"
    );

    let mut undone = moved[0].clone();
    undone.transform.center = CanvasPoint::default();
    apply(
        &mut watcher,
        InfiniteCanvasOperation::Update {
            entities: vec![undone],
        },
    );
    watcher.run();
    assert_eq!(
        watcher.rect_of(&item),
        resting,
        "a drafted change that has landed is not drawn again when the shape moves back"
    );
}
