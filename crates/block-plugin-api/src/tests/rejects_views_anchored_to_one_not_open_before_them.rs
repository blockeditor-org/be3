use super::*;

fn reported(anchored: Vec<AnchoredView>) -> FrameCommit {
    FrameCommit {
        reports: vec![report(1, anchored)],
        ..frame(1)
    }
}

#[test]
fn rejects_views_anchored_to_one_not_open_before_them() {
    let menu = |id| anchored(id, 0, ViewAnchor::Rect(Rect::ZERO), Rect::ZERO);
    let submenu = |id, parent| anchored(id, parent, ViewAnchor::Rect(Rect::ZERO), Rect::ZERO);
    assert_eq!(
        reported(vec![submenu(2, 1), menu(1)]).validate(),
        Err(DecodeError::MalformedPayload)
    );
    assert_eq!(
        reported(vec![submenu(1, 1)]).validate(),
        Err(DecodeError::MalformedPayload)
    );
    assert_eq!(
        reported(vec![menu(1), menu(1)]).validate(),
        Err(DecodeError::MalformedPayload)
    );
    assert!(
        reported(vec![menu(1), submenu(2, 1), menu(3)])
            .validate()
            .is_ok()
    );
}
