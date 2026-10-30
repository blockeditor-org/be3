use super::*;

#[test]
fn an_artifact_settings_child_round_trips() {
    let message = committed(ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        children: vec![ChildPlacement {
            child: ChildId(6),
            content: ChildContent::ArtifactSettings { block_id: [3; 16] },
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
            dialog: None,
            docks: None,
            anchor: None,
        }],
        occluders: Vec::new(),
    });
    round_trips(&message);
}
