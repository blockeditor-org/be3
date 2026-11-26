use super::*;

#[test]
fn a_reader_refuses_what_it_must_understand_and_does_not() {
    let setting = SettingV2 {
        name: String::new(),
        unit: 3,
    };
    // Field 2, must-understand, four bytes: (2 << 4) | 8 | 2.
    let bytes = encode(&setting);
    assert_eq!(bytes, [0x2a, 3, 0, 0, 0]);
    assert_eq!(decode::<SettingV2>(&bytes), Ok(setting));
    assert_eq!(decode::<SettingV1>(&bytes), Err(Error::NotUnderstood(2)));
    assert_eq!(
        decode::<SettingKeeping>(&bytes),
        Err(Error::NotUnderstood(2))
    );

    // Left out when it is its default, so older builds read the value as before.
    let plain = encode(&SettingV2 {
        name: "a".into(),
        unit: 0,
    });
    assert_eq!(
        decode::<SettingV1>(&plain),
        Ok(SettingV1 { name: "a".into() })
    );

    // An open enum keeps a variant it does not know, unless it is marked must-understand.
    assert!(matches!(
        decode::<ModeV1>(&encode(&ModeV2::On)),
        Ok(ModeV1::Other(variant)) if variant.id == 2
    ));
    assert_eq!(
        decode::<ModeV1>(&encode(&ModeV2::Strict(1))),
        Err(Error::NotUnderstood(3))
    );
    assert_eq!(
        decode::<ModeV2>(&encode(&ModeV2::Strict(1))),
        Ok(ModeV2::Strict(1))
    );
}
