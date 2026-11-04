use super::*;

#[test]
fn an_insert_after_a_deleted_element_lands_where_it_was() {
    let mut sequence = loaded("abc");
    let insert = sequence
        .insert(BOB, 2, b"X".to_vec())
        .expect("there is something to insert");
    let delete = sequence.delete(1..2).expect("there is something to delete");

    applied(&mut sequence, &delete);
    applied(&mut sequence, &insert);

    assert_eq!(text(&sequence), "aXc");
}
