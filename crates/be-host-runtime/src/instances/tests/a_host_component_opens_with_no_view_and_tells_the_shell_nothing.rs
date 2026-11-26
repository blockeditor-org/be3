use super::*;

/// A host component (the manifest's `hosts`) opens on its block with no screen to draw, and
/// what it tells the shell of a frame or a view goes nowhere (guides/apps/host_services.md,
/// "Host components").
#[test]
fn a_host_component_opens_with_no_view_and_tells_the_shell_nothing() {
    let mut instances = Instances::default();
    let host = Uuid::from_u128(7);
    let block = EditorBlock {
        id: Uuid::from_u128(1),
        block_type: Uuid::from_u128(2),
        view_block: None,
    };
    instances.report_creation(
        INSTANCE,
        Uuid::nil(),
        &Arc::new(crate::AppCatalog::default()),
        InstanceRole::Host(host, block),
    );
    let next = instances.next_screens(PASS);
    assert!(next.screens.is_empty(), "a host component gets no screen");
    let opening = next
        .opened
        .iter()
        .find_map(|message| match message {
            Outbound::Open(opening) => Some(opening),
            _ => None,
        })
        .expect("the host component opens with no view");
    assert_eq!(opening.editor, host.into_bytes());
    assert!(matches!(
        &opening.kind,
        OpeningKind::Host(open)
            if open.block_id == block.id.into_bytes()
                && open.block_type == block.block_type.into_bytes()
    ));
    assert!(
        !tell(
            &mut instances,
            ShellMessage::IntrinsicSize(Some(vec2(10.0, 10.0)))
        ),
        "a size it reports for a frame it has not changes nothing"
    );
}
