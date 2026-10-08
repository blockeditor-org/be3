use super::*;

#[test]
fn a_delete_leaves_text_inserted_inside_it_at_the_same_time() {
    let mut sequence = loaded("hello world");
    let delete = sequence.delete(0..5).expect("there is something to delete");
    let insert = sequence
        .insert(BOB, 2, b"X".to_vec())
        .expect("there is something to insert");

    applied(&mut sequence, &insert);
    applied(&mut sequence, &delete);

    assert_eq!(text(&sequence), "X world");
}
