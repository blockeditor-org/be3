use super::*;

#[test]
fn creation_children_round_trip() {
    let message = committed(ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
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
            follows_view: false,
            dialog: Some(DialogFrame {
                narrow: false,
                bottom_inset: 0.0,
            }),
            docks: None,
            anchored: None,
        }],
        occluders: Vec::new(),
    });
    round_trips(&message);
}
