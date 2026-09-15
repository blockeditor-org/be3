use super::*;

const CARD: egui::Rect = egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(110.0, 110.0));
const FRAME: egui::Rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 400.0));

fn resize(instances: &mut Instances, context: &egui::Context, rect: egui::Rect) {
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    let role = InstanceRole::Editor(EditorBlock {
        id: Uuid::nil(),
        block_type: Uuid::nil(),
    });
    instances.report(
        INSTANCE,
        REGION,
        context,
        &client,
        Uuid::nil(),
        role,
        &Arc::new(Vec::new()),
        Some(block_plugin_api::FrameSpec::default()),
        rect.size(),
        egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size()),
        1.0,
        PASS,
    );
}

#[test]
fn a_frame_takeover_keeps_the_last_painting_where_it_was() {
    let (mut instances, context, _) = placed();
    let card = (100, 100);
    let frame = (400, 400);

    assert!(
        instances
            .held(INSTANCE, REGION, Some(CARD), CARD, Some(card))
            .is_none()
    );

    instances.hold(INSTANCE, REGION);
    resize(&mut instances, &context, FRAME);
    let held = instances
        .held(INSTANCE, REGION, Some(FRAME), FRAME, Some(card))
        .expect("the painting made for the card is kept on the card");

    assert_eq!(held.rect, CARD);
    assert_eq!(held.clip, CARD);
    assert_eq!(held.drawn, card);

    assert!(
        instances
            .held(INSTANCE, REGION, Some(FRAME), FRAME, Some(frame))
            .is_none()
    );
    assert!(
        instances
            .held(INSTANCE, REGION, Some(FRAME), FRAME, Some(card))
            .is_none()
    );
}
