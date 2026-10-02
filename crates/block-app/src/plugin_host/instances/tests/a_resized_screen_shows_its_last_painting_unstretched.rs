use super::*;

const TALL: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 200.0));
const SHORT: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 120.0));

fn resize(instances: &mut Instances, rect: Rect) {
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
        rect.size(),
        Rect::from_min_size(Pos2::ZERO, rect.size()),
        1.0,
        PASS,
    );
}

#[test]
fn a_resized_screen_shows_its_last_painting_unstretched() {
    let mut instances = placed();
    let tall = (100, 200);
    let short = (100, 120);

    resize(&mut instances, TALL);
    assert!(
        instances
            .held(INSTANCE, REGION, Some(TALL), TALL, Some(tall))
            .is_none()
    );

    resize(&mut instances, SHORT);
    let held = instances
        .held(INSTANCE, REGION, Some(SHORT), SHORT, Some(tall))
        .expect("the tall painting is cropped, not squished");
    assert_eq!(held.rect, TALL);
    assert_eq!(held.shown, SHORT);
    assert_eq!(held.drawn, tall);

    assert!(
        instances
            .held(INSTANCE, REGION, Some(SHORT), SHORT, Some(short))
            .is_none()
    );

    resize(&mut instances, TALL);
    let held = instances
        .held(INSTANCE, REGION, Some(TALL), TALL, Some(short))
        .expect("the short painting is kept at its size, not stretched");
    assert_eq!(held.rect, SHORT);
    assert_eq!(held.shown, TALL);
    assert_eq!(held.drawn, short);
}
