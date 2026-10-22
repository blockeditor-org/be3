use super::*;

fn report(instances: &mut Instances) {
    let block_types = Arc::new(block_plugin_api::Catalog::default());
    instances.report(
        INSTANCE,
        REGION,
        Uuid::nil(),
        InstanceRole::Editor(
            Uuid::nil(),
            EditorBlock {
                id: Uuid::nil(),
                block_type: Uuid::nil(),
                view_block: None,
            },
        ),
        &block_types,
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
}

fn show(instances: &mut Instances) -> Vec<Message> {
    instances.show_block(
        INSTANCE,
        OpenRequest {
            id: Uuid::from_u128(7),
            block_type: Uuid::from_u128(8),
            via: None,
            editor: None,
        },
    )
}

#[test]
fn a_block_shown_before_its_instance_reports_reaches_it_once_it_does() {
    let mut instances = Instances::default();
    let shown = show(&mut instances);
    assert_eq!(shown.len(), 1);
    assert!(
        instances.hold_unreported(shown.clone()).is_empty(),
        "a message for an instance the host has not reported yet waits"
    );
    assert!(instances.take_reported().is_empty());

    report(&mut instances);
    let released = instances.take_reported();
    assert_eq!(released, shown);
    assert!(instances.take_reported().is_empty(), "it is released once");
    assert!(
        instances.gate(released).is_empty(),
        "and then waits for the plugin to open the instance"
    );
    let opened = instances.next_screens(PASS).opened;
    assert!(opened.contains(&shown[0]));

    let mut closed = Instances::default();
    let shown = show(&mut closed);
    assert!(closed.hold_unreported(shown.clone()).is_empty());
    closed.remove(INSTANCE);
    assert!(
        closed.hold_unreported(shown).is_empty(),
        "a message for a closed instance is dropped"
    );
    report(&mut closed);
    assert!(closed.take_reported().is_empty());
}
