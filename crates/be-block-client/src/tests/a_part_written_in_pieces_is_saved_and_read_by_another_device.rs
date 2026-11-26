use be_block_content::Part;
use be_type_text::TextContent;

use super::*;

/// A part written a piece at a time (`Peer::part_writer`) is the part a whole write makes of the
/// same bytes, and a commit reusing it drains with its objects, so another device reads it.
#[tokio::test]
async fn a_part_written_in_pieces_is_saved_and_read_by_another_device() {
    let harness = Harness::start().await;
    let owner = harness.owner("writer@example.com").await;
    let folder = workspace(&owner, "Parts").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();

    let segment = pseudorandom(64 * 1024, 23);
    let mut writer = owner.part_writer(block).await.unwrap();
    for piece in segment.chunks(1000) {
        writer.write(piece).unwrap();
    }
    let part = writer.finish().unwrap().part().expect("bytes were written");
    assert_eq!(part.length(), segment.len() as u64);

    let saved = owner
        .write(
            block,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                Part::node(vec![Part::from(b"a header".as_slice()), Part::Reused(part)]),
                None,
            ),
        )
        .await
        .unwrap();
    assert!(saved.published().is_some(), "{saved:?}");

    let reader = harness.second(&owner).await;
    let head = reader.manifest(block).await.unwrap().expect("a head");
    let mut expected = b"a header".to_vec();
    expected.extend_from_slice(&segment);
    assert_eq!(reader.read_manifest(&head).await.unwrap(), expected);
    assert_eq!(
        put_part(&owner, block, &segment).await.unwrap(),
        part,
        "a part put whole is the part written in pieces"
    );

    harness.stop().await;
}
