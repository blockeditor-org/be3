use super::*;
use crate::Sequence;

fn text_edit(op: SeqOp<u8>) -> Edit {
    Caption::WORDS.edit(ObjectId::ROOT, op).into()
}

fn sequence(document: &Document<Caption>) -> &Sequence<u8> {
    &document
        .rich(ObjectId::ROOT, Caption::WORDS)
        .expect("the caption has words")
        .text
}

#[test]
fn a_pending_edit_of_any_kind_carries_over_onto_content_loaded_afresh() {
    let mut before = caption("hello brave new world", Vec::new());
    for _ in 0..5 {
        before.apply(&type_into(&before, 1, 0, "scratch "));
        let erase = sequence(&before)
            .delete(0..8)
            .expect("the scratch is there");
        before.apply(&text_edit(erase));
    }
    let delete = sequence(&before).delete(12..16).expect("\"new \" is there");
    let (undelete, _) = sequence(&before)
        .inverse(&delete)
        .expect("the delete applies");
    before.apply(&text_edit(delete));
    let fresh = Document::<Caption>::from_bytes(&before.to_bytes()).expect("the bytes decode");
    assert!(fresh.session_state().len() < before.session_state().len());

    let mut old = before;
    let mut new = fresh;
    let pending: [&dyn Fn(&Document<Caption>) -> Edit; 4] = [
        &|_| text_edit(undelete.clone()),
        &|old| {
            let len = sequence(old).len();
            text_edit(
                sequence(old)
                    .move_range(0..6, len)
                    .expect("the range moves"),
            )
        },
        &|old| format(old, 2, 0..5, "bold", Some(MarkValue::Bool(true))),
        &|old| {
            let Some(mark) = old
                .rich(ObjectId::ROOT, Caption::WORDS)
                .and_then(|rich| rich.marks.marks().last().cloned())
            else {
                panic!("the bold mark is there");
            };
            Caption::WORDS.unmark(ObjectId::ROOT, mark.id).into()
        },
    ];
    for (step, made) in pending.iter().enumerate() {
        let edit = made(&old);
        let moved = Document::carry_over(&edit, &old, &new, 2)
            .unwrap_or_else(|| panic!("step {step} was dropped"));
        old.apply(&edit);
        new.apply(&moved);
        assert_eq!(words(&new), words(&old), "step {step}");
        assert_eq!(seeds(&new), seeds(&old), "step {step}");
    }
    assert_eq!(words(&new), "brave new worldhello ");
}
