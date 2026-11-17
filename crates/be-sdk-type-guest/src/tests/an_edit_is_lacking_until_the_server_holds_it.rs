use super::*;

/// What the server lacks of the edits made here: an edit is lacking until the order numbers it,
/// a replica started over carries what was lacking on, and an edit made again is followed once.
#[test]
fn an_edit_is_lacking_until_the_server_holds_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.follow_lacking(0, 1);

    sim.stalled_submits = true;
    sim.operate(0, 1, add("alone"));
    sim.settle();
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "the server lacks it until it numbers it"
    );
    sim.unstall();
    assert_eq!(sim.lacking(0, 1), 0);

    let reopened = add("reopened");
    sim.stalled_submits = true;
    sim.operate(0, 1, reopened.clone());
    sim.settle();
    sim.held.clear();
    sim.reopen(0);
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "a replica started over carries on with what the server lacked"
    );
    // The host gives again what its editors sent that the order does not hold.
    sim.send_from_editor(
        0,
        1,
        FromEditor::Operate {
            gesture: None,
            operation: ChecklistContent::encode_operation(&reopened),
        },
    );
    sim.settle();
    assert_eq!(sim.lacking(0, 1), 1, "the edit made again is followed once");
    sim.unstall();
    assert_eq!(sim.lacking(0, 1), 0, "the order holds it");
    sim.advance(crate::QUIET_INTERVAL + 1);
    assert_eq!(texts(&sim.head_content()), ["alone", "reopened"]);

    sim.open(1);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.stalled_submits = true;
    sim.operate(1, 1, add("on my way"));
    sim.operate(1, 1, add("nearly there"));
    sim.settle();
    assert_eq!(sim.lacking(1, 1), 2, "nothing numbered them yet");
    sim.unstall();
    assert_eq!(sim.lacking(1, 1), 0);
}
