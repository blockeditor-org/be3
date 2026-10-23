use super::*;

#[test]
fn forwarded_picks_and_creation_children_round_trip() {
    let filter = BlockFilter {
        name: "Block".into(),
        block_types: vec![[2; 16]],
        excluded: vec![[3; 16]],
        templates: true,
        place: Some(BlockLocation::Root),
        anchor: Some(Rect::from_min_size(
            Pos2::new(300.0, 80.0),
            Vec2::new(32.0, 32.0),
        )),
    };
    for message in [
        Message::Editor(EditorMessage::PickRequested {
            instance: EditorInstanceId(1),
            pick: 9,
            filter,
            parent: BlockLocation::Block([4; 16]),
            within: Some([8; 16]),
            touch: true,
        }),
        Message::Editor(EditorMessage::PickAnswered {
            instance: EditorInstanceId(1),
            pick: 9,
            answer: BlockPick::Chosen {
                block_id: [5; 16],
                block_type: [2; 16],
                linked: false,
                placed: true,
            },
        }),
        Message::Editor(EditorMessage::CommitChild {
            instance: EditorInstanceId(1),
            child: ChildId(6),
            parent: BlockLocation::Root,
            name: Some("Minutes".into()),
        }),
        Message::Children(ChildPlacements {
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            generation: 1,
            size: Size {
                width: 360.0,
                height: 240.0,
            },
            children: vec![ChildPlacement {
                child: ChildId(6),
                content: ChildContent::Creation {
                    editor: [7; 16],
                    template: "main".into(),
                },
                rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(320.0, 96.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(360.0, 240.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_camera: false,
            }],
            occluders: Vec::new(),
            camera: None,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
