use std::collections::BTreeMap;

use be_store::{Manifest, decode_value, encode_value};
use serde::{Serialize, de::IgnoredAny};

use super::*;

#[derive(Serialize)]
struct Grown<'a> {
    parents: &'a [CommitId],
    manifest: &'a Manifest,
    author: Uuid,
    time: i64,
    kind: &'a CommitKind,
    references: &'a [Uuid],
    #[serde(with = "be_store::bytes")]
    metadata: &'a [u8],
    signature: &'a str,
}

#[test]
fn a_commit_is_stored_by_field_name_and_reads_past_fields_it_does_not_know() {
    let store = store();
    let parent = commit(&store, "first\n", 1, None);
    let commit = Commit::new(Manifest::default(), AUTHOR, 2)
        .with_parent(Some(parent))
        .with_kind(CommitKind::Bookmark("kept".into()))
        .with_references(vec![CONTENT])
        .with_metadata(vec![1, 2, 3]);

    let fields: BTreeMap<String, IgnoredAny> =
        decode_value(&encode_value(&commit).unwrap()).unwrap();
    assert_eq!(
        fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["author", "kind", "manifest", "metadata", "parents", "references", "time"]
    );

    let grown = encode_value(&Grown {
        parents: &commit.parents,
        manifest: &commit.manifest,
        author: commit.author,
        time: commit.time,
        kind: &commit.kind,
        references: &commit.references,
        metadata: &commit.metadata,
        signature: "added by a later build",
    })
    .unwrap();
    assert_eq!(decode_value::<Commit>(&grown).unwrap(), commit);
}
