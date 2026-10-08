use super::*;

#[test]
fn a_burst_of_typing_undoes_as_one_step() {
    let mut document = note("ab");
    let mut step: Option<crate::Step> = None;
    for (at, typed) in ["x", "y", "z"].into_iter().enumerate() {
        let edit = body_edit(&document, |body| {
            body.insert(ALICE, 1 + at, typed.as_bytes().to_vec())
        });
        let next = document.step(&edit).expect("the edit changes something");
        document.apply(&edit);
        match &mut step {
            Some(step) => step.absorb(next).expect("typing absorbs"),
            None => step = Some(next),
        }
    }
    assert_eq!(body(&document), "axyzb");

    let step = step.expect("something was typed");
    document.apply(&step.undo());
    assert_eq!(body(&document), "ab");
    document.apply(&step.redo());
    assert_eq!(body(&document), "axyzb");

    let delete = body_edit(&document, |body| body.delete(0..1));
    let deleting = document.step(&delete).expect("the edit changes something");
    let mut typing = step;
    assert!(typing.absorb(deleting).is_err());
}
