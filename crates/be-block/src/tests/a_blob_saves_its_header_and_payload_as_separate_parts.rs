use super::*;

#[test]
fn a_blob_saves_its_header_and_payload_as_separate_parts() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    let content = image("frame.png", &payload);
    let encoded = content.encode();
    let start = payload_start(&encoded).unwrap();

    let parts = content.parts();
    assert_eq!(parts.flatten().unwrap(), encoded);
    let Part::Node(children) = &parts else {
        panic!("a blob is saved as a node of parts");
    };
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].length(), start as u64);

    let saved = PartRef::Chunk(be_block_store::ChunkRef {
        hash: be_block_store::Hash::of(b"the payload"),
        length: 10_000,
    });
    let header_part = PartRef::Chunk(be_block_store::ChunkRef {
        hash: be_block_store::Hash::of(b"the header"),
        length: u32::try_from(start).unwrap(),
    });
    let stored = Stored {
        bytes: encoded.clone(),
        root: Some(LoadedPart {
            reference: PartRef::Node(be_block_store::NodeRef {
                hash: be_block_store::Hash::of(b"the root"),
                length: encoded.len() as u64,
            }),
            start: 0,
            children: vec![
                LoadedPart {
                    reference: header_part,
                    start: 0,
                    children: Vec::new(),
                },
                LoadedPart {
                    reference: saved,
                    start: start as u64,
                    children: Vec::new(),
                },
            ],
        }),
    };
    let mut loaded = BytesContent::from_stored(&stored).unwrap();
    assert_eq!(loaded, content);
    loaded.apply(&BlobOp::SetHeader(header("renamed.png")));
    let Part::Node(children) = loaded.parts() else {
        panic!("a blob is saved as a node of parts");
    };
    assert_eq!(
        children[1],
        Part::Reused(saved),
        "a renamed blob keeps the payload it was loaded with"
    );
    assert_eq!(
        BytesContent::from_stored(&Stored::unsaved(encoded))
            .unwrap()
            .parts()
            .flatten()
            .unwrap(),
        content.encode()
    );
}
