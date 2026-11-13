use be_block_client::Saved;
use be_sdk_type_api::{PartRef, SearchDocument};

use super::*;

const WORDS: [&str; sections::SECTIONS] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

#[test]
fn search_text_reads_only_the_parts_asked_for_and_not_yet_indexed() {
    with_test_types(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        let head = match peer.save(block, &base(), None).await {
            Ok(Saved::Published(head)) => head,
            saved => panic!("the base publishes: {saved:?}"),
        };
        let manifest = peer
            .load_commit(head)
            .await
            .expect("the commit loads")
            .manifest;
        let parts = section_parts(&peer, &manifest).await;
        let mut search = async |keys: Option<Vec<String>>,
                                indexed: Vec<be_block_store::PartRef>| {
            replicas
                .search_text(
                    (&peer, &shared),
                    Sections::CONTENT_TYPE,
                    &manifest,
                    keys,
                    &indexed
                        .iter()
                        .map(|part| *part.hash().as_bytes())
                        .collect::<Vec<_>>(),
                )
                .await
                .expect("the type answers")
        };

        let every = search(None, Vec::new()).await;
        assert_eq!(
            every.documents,
            (0..sections::SECTIONS)
                .map(|index| SearchDocument {
                    key: index.to_string(),
                    source: Some(parts[index].into()),
                    name: None,
                    body: Some(WORDS[index].to_owned()),
                })
                .collect::<Vec<_>>(),
            "one document per section, from its part"
        );
        assert_eq!(
            every.read, parts,
            "the type loads its header and reads each part once"
        );

        let one = search(Some(vec!["3".to_owned()]), Vec::new()).await;
        assert_eq!(
            one.documents,
            vec![SearchDocument {
                key: "3".to_owned(),
                source: Some(parts[3].into()),
                name: None,
                body: Some("three".to_owned()),
            }]
        );
        assert_eq!(
            one.read,
            vec![parts[3]],
            "asked for one key, it reads only that part"
        );

        let indexed: Vec<_> = parts
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != 6)
            .map(|(_, part)| *part)
            .collect();
        let changed = search(None, indexed).await;
        assert_eq!(changed.documents.len(), sections::SECTIONS);
        for (index, document) in changed.documents.iter().enumerate() {
            assert_eq!(document.source, Some(PartRef::from(parts[index])));
            assert_eq!(
                document.body.as_deref(),
                (index == 6).then_some("six"),
                "only the section not yet indexed has a body"
            );
        }
        assert_eq!(
            changed.read,
            vec![parts[6]],
            "with every other part indexed, it reads only the one that is not"
        );
    });
}
