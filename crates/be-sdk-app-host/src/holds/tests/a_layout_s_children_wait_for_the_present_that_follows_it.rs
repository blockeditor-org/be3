use super::*;

fn commit(serial: u64, children: &[u64]) -> LayoutCommit {
    LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: children
            .iter()
            .map(|view| ChildPlacements {
                instance: EditorInstanceId(1),
                view: ViewId(*view),
                children: Vec::new(),
                occluders: Vec::new(),
            })
            .collect(),
        window_requests: Vec::new(),
        docks: Vec::new(),
    }
}

#[test]
fn a_layout_s_children_wait_for_the_present_that_follows_it() {
    let mut holds: Holds<&str> = Holds::default();
    let mut moving = commit(1, &[1]);
    assert!(
        !holds.heard_layout(&mut moving, |_| true),
        "children that only move are applied at once"
    );
    assert_eq!(moving.children.len(), 1);

    holds.heard_painted(4);
    let mut coming = commit(2, &[2]);
    assert!(holds.heard_layout(&mut coming, |_| false));
    assert!(coming.children.is_empty(), "a child that comes waits");
    holds.hold("an answer after it");
    let mut moved = commit(3, &[2]);
    holds.heard_layout(&mut moved, |_| true);
    assert!(
        moved.children.is_empty(),
        "a child already waiting waits again though it only moves"
    );
    assert!(!holds.waits_for_children(4), "the frame before the layout");
    assert!(holds.waits_for_children(5));

    holds.heard_presents(1);
    assert!(
        holds.release_presented().is_empty(),
        "the second layout's present has not come"
    );
    holds.heard_presents(1);
    let released = holds.release_presented();
    assert!(matches!(
        released.as_slice(),
        [Held::Laid(_), Held::Answer("an answer after it")]
    ));
    assert_eq!(holds.len(), 1, "the third layout waits for its own present");
}
