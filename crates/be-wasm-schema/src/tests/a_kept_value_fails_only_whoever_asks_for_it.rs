use super::*;

#[test]
fn a_kept_value_fails_only_whoever_asks_for_it() {
    let newer = HasSetting {
        setting: SettingV2 {
            name: "a".into(),
            unit: 3,
        },
    };
    let encoded = bytes(&newer);
    // A setting this build cannot read fails the whole when decoded in place...
    assert_eq!(
        kind(decode::<HasOldSetting>(&encoded)),
        ErrorKind::NotUnderstood(2)
    );
    // ...but held as Kept, only asking for it fails, and the bytes go back unchanged.
    let kept: KeepsSetting = decode(&encoded).unwrap();
    assert_eq!(kind(kept.setting.get()), ErrorKind::NotUnderstood(2));
    assert_eq!(bytes(&kept), encoded);
    let readable = KeepsSetting {
        setting: Kept::new(&SettingV1 { name: "b".into() }).unwrap(),
    };
    assert_eq!(
        decode::<KeepsSetting>(&bytes(&readable))
            .unwrap()
            .setting
            .get(),
        Ok(SettingV1 { name: "b".into() })
    );
}
