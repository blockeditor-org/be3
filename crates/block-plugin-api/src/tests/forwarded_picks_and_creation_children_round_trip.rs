use super::*;

#[test]
fn forwarded_picks_and_creation_children_round_trip() {
    let filter = BlockFilter {
        name: "Block".into(),
        block_types: vec![[2; 16]],
        excluded: vec![[3; 16]],
        templates: true,
        place: Some(BlockLocation::Root),
    };
    for message in [
        Message::Editor(EditorMessage::PickRequested {
            instance: EditorInstanceId(1),
            pick: 9,
            filter,
            parent: BlockLocation::Block([4; 16]),
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
                rect: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 96.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 360.0,
                    height: 240.0,
                },
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
            }],
            occluders: Vec::new(),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
