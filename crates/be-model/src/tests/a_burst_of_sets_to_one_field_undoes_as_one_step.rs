use super::*;

#[test]
fn a_burst_of_sets_to_one_field_undoes_as_one_step() {
    let mut document = board();
    let (_, _, write) = ids(&document);

    let mut step = undone(
        &mut document,
        &Card::TEXT.set(write, &"w".to_owned()).into(),
    );
    let next = undone(
        &mut document,
        &Card::TEXT.set(write, &"wr".to_owned()).into(),
    );
    assert!(step.absorb(next).is_ok());
    let other = document.step(&Card::DONE.set(write, &true).into()).unwrap();
    assert!(step.absorb(other).is_err());

    document.apply(&step.undo());
    assert_eq!(document.read::<Card>(write).unwrap().text, "write");
    document.apply(&step.redo());
    assert_eq!(document.read::<Card>(write).unwrap().text, "wr");
}
