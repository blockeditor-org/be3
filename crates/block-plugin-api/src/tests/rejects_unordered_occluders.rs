use super::*;

fn placements(occluders: Vec<Occluder>) -> FrameCommit {
    committed(ChildPlacements {
        instance: EditorInstanceId(1),
        region: EditorRegion::Frame,
        children: vec![ChildPlacement {
            child: ChildId(1),
            content: ChildContent::Block {
                block_id: [1; 16],
                block_type: [2; 16],
                view_block: None,
                editor: None,
            },
            rect: Rect::ZERO,
            clip: Rect::ZERO,
            own_frame: false,
            top_bar: TopBar::Hidden,
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Preview,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view: false,
        }],
        occluders,
    })
}

#[test]
fn rejects_unordered_occluders() {
    let occluder = |after| Occluder {
        after,
        rect: Rect::ZERO,
    };
    assert_eq!(
        placements(vec![occluder(2)]).validate(),
        Err(DecodeError::MalformedPayload)
    );
    assert_eq!(
        placements(vec![occluder(1), occluder(0)]).validate(),
        Err(DecodeError::MalformedPayload)
    );
    assert!(placements(vec![occluder(0), occluder(1)]).validate().is_ok());
}
