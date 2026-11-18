use be_block_content::{EditorView, EditorViewContent, ViewStateKind};
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

/// An editor writes its view state on every scroll and caret move; what its replica submits
/// while its first write is on its way holds one put for each kind, not one for each write.
#[test]
fn a_burst_of_view_state_writes_is_journaled_as_one_put_each() {
    let mut sim = Sim::<EditorViewContent>::new(1, view_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let client = Uuid::new_v4();
    for line in 0..200u32 {
        let time = u64::from(line) + 1;
        for edit in [
            sim.mirror(0, 1)
                .root()
                .set_view_state(Some(&Scrolled(line)), time, client),
            sim.mirror(0, 1)
                .root()
                .set_view_state(Some(&Caret(line / 2)), time, client),
        ] {
            if let Some(edit) = edit.expect("encodes") {
                sim.operate(0, 1, edit);
            }
        }
    }
    sim.settle();

    let operations = sim.journal_operations();
    assert!(
        operations.len() <= 2,
        "the burst goes as its first write and then one operation: {}",
        operations.len()
    );
    assert_eq!(
        operations.last().map(|operation| operation.0.len()),
        Some(2),
        "holding one put for each kind"
    );

    let content = sim.content(0);
    let view: EditorView = content.root();
    assert_eq!(view.view_state::<Scrolled>(), Some(Ok(Scrolled(199))));
    assert_eq!(view.view_state::<Caret>(), Some(Ok(Caret(99))));
}
