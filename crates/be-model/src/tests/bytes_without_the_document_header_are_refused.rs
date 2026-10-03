use super::*;

#[test]
fn bytes_without_the_document_header_are_refused() {
    let saved = board().to_bytes();
    assert_eq!(saved[..4], crate::tree::MAGIC);
    assert_eq!(saved[4], crate::tree::FORMAT);
    assert!(Document::<Board>::from_bytes(&saved).is_ok());

    assert!(Document::<Board>::from_bytes(&saved[5..]).is_err());

    let mut later = saved.clone();
    later[4] = crate::tree::FORMAT + 1;
    assert!(Document::<Board>::from_bytes(&later).is_err());
}
