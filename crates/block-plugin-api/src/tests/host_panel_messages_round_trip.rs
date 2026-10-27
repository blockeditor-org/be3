use super::*;

#[test]
fn host_panel_messages_round_trip() {
    for panel in HostPanel::ALL {
        let message = committed(ChildPlacements {
            instance: EditorInstanceId(3),
            region: EditorRegion::Frame,
            children: vec![ChildPlacement {
                child: ChildId(1),
                content: ChildContent::Host(panel),
                rect: Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(520.0, 420.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0)),
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
                docks: false,
            }],
            occluders: Vec::new(),
        });
        round_trips(&message);
    }
}
