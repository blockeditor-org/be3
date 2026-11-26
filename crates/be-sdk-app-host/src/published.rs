//! Which of the app protocol's interfaces an app reaches as published ones
//! (`be_sdk_api::is_published`): the watches and calls of extensions its manifest may be granted.

use be_sdk_api::ops::{MethodId, Started, Watching};

/// The published watch `method` opens or asks the next value of, if it is one.
pub fn watching(method: MethodId) -> Option<Watching> {
    be_sdk_api::embed::WATCHES
        .iter()
        .find(|watching| {
            (watching.open == method || watching.next == method)
                && be_sdk_api::is_published(watching.interface)
        })
        .copied()
}

/// The published interface a call belongs to, if it is a published one's.
pub fn interface_of(started: &Started) -> Option<&'static str> {
    let (interface, _) = be_sdk_api::embed::method_name(started.method)?.split_once('#')?;
    be_sdk_api::is_published(interface).then_some(interface)
}
