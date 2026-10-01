use super::*;

fn report(instances: &mut Instances, size: Vec2) {
    let role = InstanceRole::Editor(EditorBlock {
        id: Uuid::nil(),
        block_type: Uuid::nil(),
        view_block: None,
    });
    instances.report(
        INSTANCE,
        REGION,
        Uuid::nil(),
        role,
        &Arc::new(Vec::new()),
        Some(block_plugin_api::FrameSpec::default()),
        size,
        Rect::from_min_size(Pos2::ZERO, size),
        1.0,
        PASS,
    );
}

#[test]
fn a_screen_that_changes_size_asks_for_another_pass() {
    let mut instances = placed();
    instances.take_resized();

    report(&mut instances, SIZE);
    assert!(!instances.take_resized());

    report(&mut instances, vec2(100.0, 60.0));
    assert!(instances.take_resized());
    assert!(!instances.take_resized());
}
