use super::*;
use block_plugin_api::blocks::ShowRequest;

fn report(instances: &mut Instances) {
    let block_types = Arc::new(crate::editors::PluginCatalog::default());
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

fn show(instances: &mut Instances) {
    instances.show_block(
        INSTANCE,
        OpenRequest {
            id: Uuid::from_u128(7),
            block_type: Uuid::from_u128(8),
            via: None,
            editor: None,
        },
    );
}

fn shows(instances: &mut Instances) -> OpId {
    open_watch(instances, |op| Call::ShowRequestsNew {
        resource: wire::ShowRequests(op),
    })
}

#[test]
fn a_block_shown_before_its_instance_reports_reaches_it_once_it_does() {
    let mut instances = Instances::default();
    show(&mut instances);
    report(&mut instances);
    let watch = shows(&mut instances);
    let shown: Option<Vec<ShowRequest>> = answered(&mut instances, watch);
    assert_eq!(
        shown,
        Some(vec![ShowRequest::Block(
            block_plugin_api::blocks::OpenRequest {
                block_id: Uuid::from_u128(7).into_bytes(),
                block_type: Uuid::from_u128(8).into_bytes(),
                via: None,
                editor: None,
            }
        )]),
        "a block shown before its instance reported waits for it"
    );
    assert!(
        answered::<Vec<ShowRequest>>(&mut instances, watch).is_none(),
        "it is shown once"
    );

    let mut closed = Instances::default();
    show(&mut closed);
    closed.remove(INSTANCE);
    show(&mut closed);
    report(&mut closed);
    let watch = shows(&mut closed);
    assert!(
        answered::<Vec<ShowRequest>>(&mut closed, watch).is_none(),
        "a block shown to a closed instance is dropped"
    );
}
