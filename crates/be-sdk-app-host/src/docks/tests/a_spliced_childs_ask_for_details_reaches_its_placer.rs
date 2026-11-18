use super::*;

#[test]
fn a_spliced_childs_ask_for_details_reaches_its_placer() {
    let mut docks = nested();
    let _ = updates(&mut docks, "shell");
    assert!(docks.details(INNER));
    assert_eq!(
        docks.take_details("outer"),
        [(OUTER, INNER_CHILD)],
        "the frame that would carry it is not shown, so the host passes it on"
    );
    assert!(docks.take_details("outer").is_empty());

    docks.heard("inner", vec![declare(INNER, None)]);
    assert!(
        !docks.details(INNER),
        "a child that declares no dock is shown as a frame, which carries it"
    );
}
