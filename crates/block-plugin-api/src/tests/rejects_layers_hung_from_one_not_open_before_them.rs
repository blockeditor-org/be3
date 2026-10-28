use super::*;

fn reported(layers: Vec<Layer>) -> FrameCommit {
    FrameCommit {
        reports: vec![report(1, layers)],
        ..frame(1)
    }
}

#[test]
fn rejects_layers_hung_from_one_not_open_before_them() {
    let menu = |id| {
        layer(
            id,
            LayerParent::Region,
            LayerPlacement::Anchored(Rect::ZERO),
            Rect::ZERO,
        )
    };
    let submenu = |id, parent| {
        layer(
            id,
            LayerParent::Layer(LayerId(parent)),
            LayerPlacement::Anchored(Rect::ZERO),
            Rect::ZERO,
        )
    };
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
