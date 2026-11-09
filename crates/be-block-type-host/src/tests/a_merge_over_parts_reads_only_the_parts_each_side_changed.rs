use std::collections::HashSet;

use be_block_client::{Saved, Write};
use be_sdk_type_api::WrittenPart;

use super::*;

#[test]
fn a_merge_over_parts_reads_only_the_parts_each_side_changed() {
    with_sections(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        let head = match peer.save(block, &base(), None).await {
            Ok(Saved::Published(head)) => head,
            saved => panic!("the base publishes: {saved:?}"),
        };
        let base = peer
            .load_commit(head)
            .await
            .expect("the base loads")
            .manifest;
        let ours = written(&peer, block, &[(2, "TWO")]).await;
        let theirs = written(&peer, block, &[(5, "FIVE")]).await;
        let [base_parts, ours_parts, theirs_parts] = [
            section_parts(&peer, &base).await,
            section_parts(&peer, &ours).await,
            section_parts(&peer, &theirs).await,
        ];
        let unchanged = |index: usize| index != 2 && index != 5;
        for index in (0..sections::SECTIONS).filter(|index| unchanged(*index)) {
            assert_eq!(base_parts[index], ours_parts[index]);
            assert_eq!(base_parts[index], theirs_parts[index]);
        }

        let merged = replicas
            .merge(
                (&peer, &shared),
                Sections::CONTENT_TYPE,
                [Some(&base), Some(&ours), Some(&theirs)],
            )
            .await
            .expect("the sides load")
            .expect("the type merges");
        assert_eq!(merged.conflicts, 0);

        let ours_and_base: HashSet<_> = [&base_parts, &ours_parts, &theirs_parts]
            .into_iter()
            .flatten()
            .copied()
            .collect();
        let read: HashSet<_> = sections::parts_read()
            .into_iter()
            .filter(|part| ours_and_base.contains(part))
            .collect();
        assert_eq!(
            read,
            HashSet::from([ours_parts[2], theirs_parts[5], base_parts[2], base_parts[5]]),
            "the merge read the part each side changed and the base's of those, nothing else"
        );
        let reused: Vec<_> = merged
            .content
            .parts
            .iter()
            .filter_map(|part| match part {
                WrittenPart::Reused(part) => Some(be_block_store::PartRef::from(*part)),
                _ => None,
            })
            .collect();
        let kept: Vec<_> = (0..sections::SECTIONS)
            .filter(|index| unchanged(*index))
            .map(|index| base_parts[index])
            .collect();
        assert_eq!(
            reused, kept,
            "the merge reuses the eight parts no side changed"
        );

        let published = match peer
            .write(
                block,
                Write::encoded(Sections::CONTENT_TYPE, merged.parts(), Some(head)).described(
                    merged.content.references.clone(),
                    merged.content.derived.clone(),
                ),
            )
            .await
        {
            Ok(Saved::Published(published)) => published,
            saved => panic!("the merge publishes: {saved:?}"),
        };
        let manifest = peer
            .load_commit(published)
            .await
            .expect("the merge loads")
            .manifest;
        let parts = section_parts(&peer, &manifest).await;
        for index in (0..sections::SECTIONS).filter(|index| unchanged(*index)) {
            assert_eq!(
                parts[index], base_parts[index],
                "section {index} is the base's part"
            );
        }
        let content = Sections::decode(&peer.read_manifest(&manifest).await.expect("it reads"))
            .expect("it decodes");
        assert_eq!(content.section(2), Some(&b"TWO"[..]));
        assert_eq!(content.section(5), Some(&b"FIVE"[..]));
        assert_eq!(content.section(7), Some(&b"seven"[..]));
    });
}
