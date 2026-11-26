use super::*;

#[test]
fn a_reader_refuses_what_it_must_understand_and_does_not() {
    let setting = SettingV2 {
        name: String::new(),
        unit: 3,
    };
    // Field 2, must-understand, four bytes: (2 << 6) | (1 << 4) | 3.
    let encoded = bytes(&setting);
    assert_eq!(encoded, [0x93, 0x01, 3, 0, 0, 0]);
    assert_eq!(decode::<SettingV2>(&encoded), Ok(setting));
    assert_eq!(
        kind(decode::<SettingV1>(&encoded)),
        ErrorKind::NotUnderstood(2)
    );
    assert_eq!(
        kind(decode::<SettingKeeping>(&encoded)),
        ErrorKind::NotUnderstood(2)
    );

    // Left out when it is its default, so older builds read the value as before.
    let plain = bytes(&SettingV2 {
        name: "a".into(),
        unit: 0,
    });
    assert_eq!(
        decode::<SettingV1>(&plain),
        Ok(SettingV1 { name: "a".into() })
    );

    // An open enum keeps a variant it does not know, unless it is marked must-understand.
    assert!(matches!(
        decode::<ModeV1>(&bytes(&ModeV2::On)),
        Ok(ModeV1::Other(variant)) if variant.id == 2
    ));
    assert_eq!(
        kind(decode::<ModeV1>(&bytes(&ModeV2::Strict(1)))),
        ErrorKind::NotUnderstood(3)
    );
    assert_eq!(
        decode::<ModeV2>(&bytes(&ModeV2::Strict(1))),
        Ok(ModeV2::Strict(1))
    );
}
