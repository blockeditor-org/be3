use super::*;

const CARD: Rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(110.0, 110.0));
const FRAME: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 400.0));

fn resize(instances: &mut Instances, rect: Rect) {
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    instances.report(
        INSTANCE,
        View::MAIN,
        Uuid::nil(),
        role,
        &Arc::new(crate::PluginCatalog::default()),
        Some(block_plugin_api::FrameSpec::default()),
        rect.size(),
        Rect::from_min_size(Pos2::ZERO, rect.size()),
        1.0,
        PASS,
    );
}

#[test]
fn a_frame_takeover_keeps_the_last_painting_where_it_was() {
    let mut instances = placed();
    let card = (100, 100);
    let frame = (400, 400);

    assert!(
        instances
            .held(INSTANCE, REGION, Some(CARD), CARD, Some((card, 1.0)))
            .is_none()
    );

    instances.hold(INSTANCE, REGION);
    resize(&mut instances, FRAME);
    let held = instances
        .held(INSTANCE, REGION, Some(FRAME), FRAME, Some((card, 1.0)))
        .expect("the painting made for the card is kept on the card");

    assert_eq!(held.rect, CARD);
    assert_eq!(held.drawn, card);

    assert!(
        instances
            .held(INSTANCE, REGION, Some(FRAME), FRAME, Some((frame, 1.0)))
            .is_none()
    );
    assert!(
        instances
            .held(INSTANCE, REGION, Some(FRAME), FRAME, Some((card, 1.0)))
            .is_none()
    );
}
