use super::*;

#[test]
fn a_malformed_descriptor_is_refused() {
    let mut guest = Guest::new();
    let (pointer, length) = guest.place(64, &[0xff; 3]);
    assert_eq!(glue::create_buffer(&mut guest, pointer, length), 0);
    assert_eq!(guest.refused.len(), 1);
    assert!(
        guest.refused[0].contains("create-buffer"),
        "{:?}",
        guest.refused
    );
}
