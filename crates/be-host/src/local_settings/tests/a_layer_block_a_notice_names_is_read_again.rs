use super::*;

#[test]
fn a_layer_block_a_notice_names_is_read_again() {
    let (every, this) = (Uuid::new_v4(), Uuid::new_v4());
    let mut sync = SettingsSync::<DisplaySettings>::default();
    sync.set_layers(Layers {
        every_device: Some(every),
        this_device: Some(this),
    });
    sync.due = false;

    sync.heard(&be_host_stack::Changes {
        blocks: [Uuid::new_v4()].into_iter().collect(),
        ..be_host_stack::Changes::default()
    });
    assert!(
        !sync.due,
        "a notice about another block reads nothing again"
    );

    sync.heard(&be_host_stack::Changes {
        blocks: [this].into_iter().collect(),
        ..be_host_stack::Changes::default()
    });
    assert!(
        sync.due,
        "a notice about a layer's block reads the layers again"
    );
}
