use be_sdk_type_api::{Failure, Loaded, OpError, embed::exports::replicas as module};

use super::*;
use crate::{OUTSIDE_REQUEST, Scope};

/// What the probe's merge failed with, which names what the host answered it.
fn refused(answer: Result<Result<be_sdk_type_api::MergedParts, Failure>, OpError>) -> String {
    match answer {
        Ok(Err(Failure::Malformed(error))) => error,
        other => panic!("the probe always fails: {other:?}"),
    }
}

fn assert_refused(error: &str, host: &str) {
    assert!(
        error.contains(OUTSIDE_REQUEST),
        "{host} refuses a read of another block's part: {error}"
    );
    assert!(
        error.contains("call: Err(Canceled)"),
        "{host} refuses every call but read-part under a stateless request: {error}"
    );
}

#[test]
fn a_stateless_request_reads_only_parts_of_the_commits_it_was_handed() {
    with_test_types(async |peer, shared, mut replicas| {
        let other = block(&peer).await;
        let foreign = section_parts(&peer, &written(&peer, other, &[]).await).await[4];
        let probed = peer
            .create::<Probe>(BlockParent::Root)
            .await
            .expect("the probe is made");
        let manifest = peer
            .write_content(
                probed,
                Probe::CONTENT_TYPE,
                Probe {
                    target: Some(foreign),
                }
                .encode(),
            )
            .await
            .expect("the probe is written");
        let loaded = Loaded::of(
            peer.load_first_part(&manifest)
                .await
                .expect("the probe loads"),
        );
        let organization = peer.organization();
        let module = replicas
            .modules
            .module_of(Probe::CONTENT_TYPE)
            .expect("the probe has a module");
        let ask = |connection: &mut be_sdk_type_api::ops::Connection| {
            module::merge_parts(
                connection,
                Probe::CONTENT_TYPE,
                None,
                Some(loaded.clone()),
                None,
                organization,
            )
        };

        let (answer, scope) = replicas
            .ask_reading((&peer, &shared), module, Scope::of([&loaded]), ask)
            .await;
        assert_refused(&refused(answer), "a host");
        assert!(scope.read().is_empty(), "nothing was read: {scope:?}");

        let mut scope = Scope::of([&loaded]);
        let answer = replicas
            .modules
            .ask_reading(module, &*peer, &mut scope, ask)
            .await;
        assert_refused(&refused(answer), "be sync");
        assert!(scope.read().is_empty(), "nothing was read: {scope:?}");
    });
}
