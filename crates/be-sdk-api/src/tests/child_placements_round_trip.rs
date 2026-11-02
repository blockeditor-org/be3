use super::*;

#[test]
fn child_placements_round_trip() {
    let message = committed(ChildPlacements {
        instance: EditorInstanceId(4),
        view: ViewId::MAIN,
        children: vec![
            ChildPlacement {
                child: ChildId(1),
                content: ChildContent::Block {
                    block_id: [3; 16],
                    block_type: [5; 16],
                    view_block: None,
                    editor: None,
                },
                rect: Rect::from_min_size(Pos2::new(12.0, 24.0), Vec2::new(320.0, 180.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                own_frame: true,
                top_bar: TopBar::Shown,
                corner_radius: 6.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Passive,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: true,
                dialog: None,
                docks: None,
                anchor: Some(Rect::from_min_size(
                    Pos2::new(0.0, -28.0),
                    Vec2::new(90.0, 28.0),
                )),
            },
            ChildPlacement {
                child: ChildId(2),
                content: ChildContent::Block {
                    block_id: [7; 16],
                    block_type: [5; 16],
                    view_block: None,
                    editor: None,
                },
                rect: Rect::from_min_size(Pos2::new(12.0, 220.0), Vec2::new(320.0, 180.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Above,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
                dialog: None,
                docks: None,
                anchor: None,
            },
            ChildPlacement {
                child: ChildId(3),
                content: ChildContent::Block {
                    block_id: [7; 16],
                    block_type: [5; 16],
                    view_block: None,
                    editor: None,
                },
                rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 32.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
                dialog: None,
                docks: None,
                anchor: None,
            },
        ],
        occluders: vec![Occluder {
            after: 1,
            rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(64.0, 480.0)),
        }],
    });
    round_trips(&message);

    let placements = &message.children[0];
    assert!(placements.occluded(0, Pos2::new(8.0, 8.0)));
    assert!(!placements.occluded(1, Pos2::new(8.0, 8.0)));
}
