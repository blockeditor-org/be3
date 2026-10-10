use super::*;

#[test]
fn the_saved_form_is_only_the_visible_items() {
    let mut sequence = loaded("hello");
    typed(&mut sequence, ALICE, 5, "!");
    let delete = sequence.delete(0..1).expect("there is something to delete");
    applied(&mut sequence, &delete);

    let saved = postcard::to_stdvec(&sequence).expect("the sequence encodes");
    let reloaded: Sequence<u8> = postcard::from_bytes(&saved).expect("the sequence decodes");

    assert_eq!(
        saved,
        postcard::to_stdvec(&b"ello!".to_vec()).expect("the bytes encode")
    );
    assert_eq!(reloaded, sequence);
    assert!(reloaded.is_fresh());
}
