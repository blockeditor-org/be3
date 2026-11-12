use super::*;

#[test]
fn approving_a_device_offline_says_it_needs_a_connection() {
    let mut shown = Shown::pairing(HostUi {
        pairing: vec![crate::ui::PairingRow {
            from: 1,
            device: "Phone".to_owned(),
            refused: be_host_stack::OFFLINE_PAIRING.to_owned(),
        }],
        ..Default::default()
    });
    assert!(
        shown
            .stepped
            .document()
            .find_test_id("host.pairing.refused")
            .is_some(),
        "the dialog where the device was approved says why it was not"
    );
    shown.painted("approving_a_device_offline_says_it_needs_a_connection");
}
