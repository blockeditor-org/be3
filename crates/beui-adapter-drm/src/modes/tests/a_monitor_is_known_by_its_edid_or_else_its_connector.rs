use super::*;

fn edid(name: Option<&str>, serial: Option<&str>, number: u32) -> Vec<u8> {
    let mut edid = vec![0; 128];
    edid[..8].copy_from_slice(&HEADER);
    let letters = b"DEL".map(|letter| u16::from(letter - b'@'));
    let packed = (letters[0] << 10) | (letters[1] << 5) | letters[2];
    edid[8..10].copy_from_slice(&packed.to_be_bytes());
    edid[10..12].copy_from_slice(&0xa0c3_u16.to_le_bytes());
    edid[12..16].copy_from_slice(&number.to_le_bytes());
    edid[54] = 1;
    let mut descriptor = 72;
    for (tag, text) in [(0xfc, name), (0xff, serial)] {
        let Some(text) = text else { continue };
        edid[descriptor + 3] = tag;
        let mut field = [b' '; 13];
        field[..text.len()].copy_from_slice(text.as_bytes());
        if text.len() < 13 {
            field[text.len()] = b'\n';
        }
        edid[descriptor + 5..descriptor + 18].copy_from_slice(&field);
        descriptor += 18;
    }
    edid
}

#[test]
fn a_monitor_is_known_by_its_edid_or_else_its_connector() {
    let named = identity(&edid(Some("DELL AW2524H"), Some("7XQ2B34"), 1)).unwrap();
    assert_eq!(
        named,
        Identity {
            make: "DEL".to_owned(),
            model: "DELL AW2524H".to_owned(),
            serial: "7XQ2B34".to_owned(),
            name: Some("DELL AW2524H".to_owned()),
        }
    );
    assert_eq!(monitor_id(Some(&named), "DP-1"), "DEL|DELL AW2524H|7XQ2B34");
    assert_eq!(monitor_name(Some(&named), "DP-1"), "DELL AW2524H");

    let bare = identity(&edid(None, None, 4242)).unwrap();
    assert_eq!(monitor_id(Some(&bare), "DP-1"), "DEL|A0C3|4242");
    assert_eq!(monitor_name(Some(&bare), "DP-1"), "DEL A0C3");

    assert_eq!(identity(&[0; 128]), None);
    assert_eq!(identity(&HEADER), None);
    assert_eq!(monitor_id(None, "HDMI-A-1"), "HDMI-A-1");
    assert_eq!(monitor_name(None, "HDMI-A-1"), "HDMI-A-1");
}
