use super::*;

#[test]
fn an_object_in_an_unknown_format_is_told_apart_from_corruption() {
    let vault = vault(6);
    let sealed = vault.seal(b"the quick brown fox");
    assert_eq!(sealed[0], vault::SEALED_FORMAT);

    let mut later = sealed.clone();
    later[0] = vault::SEALED_FORMAT + 1;
    assert!(matches!(
        vault.open(&later),
        Err(StoreError::UnknownFormat(format)) if format == vault::SEALED_FORMAT + 1
    ));

    assert!(matches!(vault.open(&[]), Err(StoreError::Corrupt)));
}
