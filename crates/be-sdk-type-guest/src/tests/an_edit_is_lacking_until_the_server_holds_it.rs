use super::*;

#[test]
fn an_edit_is_lacking_until_the_server_holds_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.follow_lacking(0, 1);

    sim.operate(0, 1, add("alone"));
    sim.settle();
    assert_eq!(
        sim.editors[&(0, 1)].pending(),
        0,
        "the replica confirmed the edit"
    );
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "alone, the owner appends within APPEND_INTERVAL, and the server lacks it until then"
    );
    sim.advance(crate::APPEND_INTERVAL + 1);
    assert_eq!(sim.lacking(0, 1), 0);

    let reopened = add("reopened");
    sim.operate(0, 1, reopened.clone());
    sim.settle();
    sim.reopen(0);
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "a replica started over carries on with what the server lacked"
    );
    // The host replays what its editors sent since the last seal, as it does on a reopen.
    sim.send_from_editor(
        0,
        1,
        FromEditor::Operate(ChecklistContent::encode_operation(&reopened)),
    );
    sim.settle();
    assert_eq!(sim.lacking(0, 1), 1, "the edit made again is followed once");
    sim.advance(crate::QUIET_INTERVAL + 1);
    assert_eq!(sim.lacking(0, 1), 0, "a seal holds it");
    assert_eq!(texts(&sim.head_content()), ["alone", "reopened"]);

    sim.open(1);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("on my way"));
    sim.operate(1, 1, add("nearly there"));
    sim.settle();
    assert_eq!(
        sim.lacking(1, 1),
        2,
        "no owner accepted the follower's edits, so the server lacks them"
    );
    sim.deaf.remove(&0);
    sim.resynchronize(1);
    sim.settle();
    assert_eq!(sim.lacking(1, 1), 0);
}
