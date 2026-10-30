use super::*;
use be_block::BlockContent;
use counter_types::CounterContent;

use std::time::Duration;

use block_type_guest::editor::FromEditor;

fn opened_first(opened: &mut HashSet<EditorInstanceId>, sent: &[Outbound]) {
    for outbound in sent {
        match outbound {
            Outbound::Open(opening) => {
                assert!(opened.insert(opening.instance()), "an editor opened twice");
            }
            outbound => {
                if let Some(instance) = outbound.instance() {
                    assert!(
                        opened.contains(&instance),
                        "{outbound:?} reached an editor before it opened"
                    );
                }
            }
        }
    }
}

#[test]
fn an_editor_is_only_sent_messages_its_plugin_session_accepts() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let block_type = CounterContent::CONTENT_TYPE;
    let mut instances = placed_on(block, block_type);
    let mut open = HashSet::default();
    let opened = next_screens(&mut instances).opened;
    assert!(
        opened
            .iter()
            .any(|message| matches!(message, Outbound::Open(_)))
    );
    opened_first(&mut open, &opened);
    to_replica(&mut instances, block, &FromEditor::Resend);
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&block).then_some(())
    })
    .expect("the new stack never held the block's content");
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        shared.editor_frames.contains_key(&INSTANCE.0).then_some(())
    })
    .expect("the replica never answered the editor");

    let carried = next_screens(&mut instances).opened;

    assert!(
        !from_replica(&mut instances, block).is_empty(),
        "the host never sent the block's content"
    );
    opened_first(&mut open, &carried);
}
