use super::*;

#[test]
fn a_no_copy_field_is_read_past_but_never_written_back() {
    let setting = SettingV3 {
        name: "a".into(),
        checksum: 99,
    };
    let encoded = bytes(&setting);
    assert_eq!(
        decode::<SettingV1>(&encoded),
        Ok(SettingV1 { name: "a".into() })
    );
    let kept: SettingKeeping = decode(&encoded).unwrap();
    assert!(kept.rest.is_empty());
    assert_eq!(bytes(&kept), bytes(&SettingV1 { name: "a".into() }));
}
