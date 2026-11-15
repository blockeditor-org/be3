use be_block_content::{EditorViewContent, ViewStateKind};
use be_block_session::{Payload, SessionMessage};
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Scrolled(u32);

impl ViewStateKind for Scrolled {
    const ID: Uuid = Uuid::from_u128(1);
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Caret(u32);

impl ViewStateKind for Caret {
    const ID: Uuid = Uuid::from_u128(2);
}

fn view_module() -> Module {
    Module::new(vec![Registration::of::<EditorViewContent>()])
}

/// A follower holds back what it writes while its first submit is unanswered, folding the
/// writes into one: a burst to two kinds of view state goes to the owner as one put each.
#[test]
fn a_followers_burst_of_view_state_writes_is_sent_as_one_put_each() {
    let mut sim = Sim::<EditorViewContent>::new(2, view_module);
    sim.open(0);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.settle();
    let client = Uuid::new_v4();
    for line in 0..200u32 {
        let time = u64::from(line) + 1;
        for edit in [
            sim.mirror(1, 1)
                .root()
                .set_view_state(Some(&Scrolled(line)), time, client),
            sim.mirror(1, 1)
                .root()
                .set_view_state(Some(&Caret(line / 2)), time, client),
        ] {
            if let Some(edit) = edit.expect("encodes") {
                sim.operate(1, 1, edit);
            }
        }
    }
    sim.settle();

    let submitted: Vec<Edit> = sim
        .sent
        .iter()
        .filter(|(device, _)| *device == 1)
        .filter_map(|(_, bytes)| {
            let crate::wire::Wire::Session(SessionMessage::Submit {
                payload: Payload::Inline(bytes),
                ..
            }) = crate::wire::Wire::decode(bytes)?
            else {
                return None;
            };
            <EditorViewContent as LiveEdit>::decode_operation(&bytes).ok()
        })
        .collect();
    assert!(
        (1..=3).contains(&submitted.len()),
        "the burst is sent as a few submits, not {}",
        submitted.len()
    );
    assert!(
        submitted.iter().all(|edit| edit.0.len() <= 2),
        "each submit holds at most one put for each kind: {:?}",
        submitted
            .iter()
            .map(|edit| edit.0.len())
            .collect::<Vec<_>>()
    );

    for device in 0..2 {
        let view = sim.content(device).root();
        assert_eq!(view.view_state::<Scrolled>(), Some(Ok(Scrolled(199))));
        assert_eq!(view.view_state::<Caret>(), Some(Ok(Caret(99))));
    }
}
