use super::*;

#[test]
fn adopting_session_state_carries_series_positions_to_a_reloaded_document() {
    let mut document = trace(&[1.0]);
    document.apply(&Edit(vec![grown(&document, &[1.0, 2.0])]));
    let mut reloaded =
        Document::<Trace>::from_bytes(&document.to_bytes()).expect("the bytes decode");
    reloaded
        .adopt_session_state(&document.session_state())
        .expect("the state fits the reloaded points");

    let change = grown(&document, &[1.0, 2.0, 3.0]);
    document.apply(&Edit(vec![change.clone()]));
    reloaded.apply(&Edit(vec![change]));
    assert_eq!(reloaded, document);
}
