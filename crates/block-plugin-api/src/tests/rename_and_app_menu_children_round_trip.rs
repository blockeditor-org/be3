use super::*;

#[test]
fn rename_and_app_menu_children_round_trip() {
    let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(280.0, 120.0));
    let child = |child, content| ChildPlacement {
        child: ChildId(child),
        content,
        rect,
        clip: rect,
        own_frame: false,
        top_bar: TopBar::Hidden,
        corner_radius: 8.0,
        layer: ChildLayer::Above,
        mode: ChildMode::Live,
        intrinsic: None,
        rotation: 0.0,
        opacity: 1.0,
        follows_view: false,
        dialog: None,
        docks: None,
    };
    let mut message = committed(ChildPlacements {
        instance: EditorInstanceId(3),
        view: ViewId::MAIN,
        children: vec![
            child(1, ChildContent::Rename([1; 16])),
            child(2, ChildContent::Host(HostPanel::AppMenu)),
        ],
        occluders: Vec::new(),
    });
    round_trips(&message);

    message.visit_block_ids(&mut |instance, role, block_id| {
        assert_eq!(
            (instance, role),
            (EditorInstanceId(3), BlockIdRole::Existing)
        );
        *block_id = [5; 16];
    });
    assert_eq!(
        message.children[0].children[0].content,
        ChildContent::Rename([5; 16])
    );
}
