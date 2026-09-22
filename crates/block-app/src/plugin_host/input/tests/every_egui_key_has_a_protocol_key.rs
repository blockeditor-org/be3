use super::*;

#[test]
fn every_egui_key_has_a_protocol_key() {
    for key in egui::Key::ALL {
        assert_eq!(format!("{:?}", protocol_key(*key)), format!("{key:?}"));
    }
}
