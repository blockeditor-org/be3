use be_sdk_api::{PushDeviceWatch, Watched};

use super::*;

fn published() -> Option<Vec<u8>> {
    crate::watches::published()
        .into_iter()
        .find(|(open, _)| *open == PushDeviceWatch::OPEN)
        .map(|(_, value)| value.to_vec())
}

/// Apps hear whether this device shows pushes as the host learns it, with nothing in the host's
/// frame looking for a change.
#[test]
fn the_state_is_published_as_it_changes() {
    publish();
    assert_eq!(
        published(),
        Some(be_sdk_api::ops::encode(&None::<be_sdk_api::PushDevice>)),
        "a host that cannot receive pushes says so from the start"
    );

    set_state(PushDevice::On);
    assert_eq!(
        published(),
        Some(be_sdk_api::ops::encode(&Some(be_sdk_api::PushDevice::On)))
    );

    set_state(PushDevice::Denied);
    assert_eq!(
        published(),
        Some(be_sdk_api::ops::encode(&Some(be_sdk_api::PushDevice::Denied)))
    );
}
