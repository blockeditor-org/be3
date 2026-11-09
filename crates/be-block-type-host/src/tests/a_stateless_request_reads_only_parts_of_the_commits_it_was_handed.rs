use be_sdk_type_api::{Failure, Loaded, embed::exports::replicas as module};

use super::*;
use crate::{OUTSIDE_REQUEST, Scope};

#[test]
fn a_stateless_request_reads_only_parts_of_the_commits_it_was_handed() {
    with_sections(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        let base = written(&peer, block, &[]).await;
        let ours = written(&peer, block, &[(3, "THREE")]).await;
        let load = async |manifest: &Manifest| {
            Loaded::of(
                peer.load_first_part(manifest)
                    .await
                    .expect("the first part loads"),
            )
        };
        let (base, ours) = (load(&base).await, load(&ours).await);
        let organization = peer.organization();
        let module = replicas
            .modules
            .module_of(Sections::CONTENT_TYPE)
            .expect("the type has a module");

        let named_base_only = Scope::of([&base]);
        let refused = replicas
            .ask_reading((&peer, &shared), module, named_base_only, |connection| {
                module::merge_parts(
                    connection,
                    Sections::CONTENT_TYPE,
                    Some(base.clone()),
                    Some(ours.clone()),
                    Some(base.clone()),
                    organization,
                )
            })
            .await
            .expect("the module answers");
        match refused {
            Err(Failure::Malformed(error)) => assert!(
                error.contains(OUTSIDE_REQUEST),
                "reading a part of the commit the request did not name is refused: {error}"
            ),
            other => panic!("the merge reads a part it was not handed: {other:?}"),
        }

        let both = Scope::of([&base, &ours]);
        let merged = replicas
            .ask_reading((&peer, &shared), module, both, |connection| {
                module::merge_parts(
                    connection,
                    Sections::CONTENT_TYPE,
                    Some(base.clone()),
                    Some(ours.clone()),
                    Some(base.clone()),
                    organization,
                )
            })
            .await
            .expect("the module answers");
        assert!(
            matches!(merged, Ok(ref merged) if merged.conflicts == 0),
            "with both commits named the same merge reads what it needs: {merged:?}"
        );
    });
}
