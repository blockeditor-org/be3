use super::*;

#[test]
fn a_part_written_in_pieces_seals_as_a_whole_write_does() {
    let vault = vault(1);
    let data = pseudorandom(64 * 1024, 5);
    let whole = vault.write(id(1), CONTENT, &data).unwrap().root;
    assert!(matches!(whole, Some(PartRef::Chunks(_))));
    for piece in [1, 7, 1000, 1024, 1025, 5000, data.len()] {
        let mut writer = vault.part_writer(id(1)).unwrap();
        let mut most = 0;
        for bytes in data.chunks(piece) {
            writer.write(bytes).unwrap();
            most = most.max(writer.held());
        }
        assert!(
            most <= ChunkerConfig::SMALL.maximum,
            "pieces of {piece} held {most} bytes"
        );
        assert_eq!(writer.finish().unwrap().part(), whole, "pieces of {piece}");
    }

    let mut cut = Vec::new();
    let mut at = 0;
    let mut splitter = Splitter::new(ChunkerConfig::SMALL);
    for bytes in data.chunks(333) {
        splitter
            .push(bytes, |chunk| {
                cut.push(at..at + chunk.len());
                at += chunk.len();
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    splitter
        .finish(|chunk| {
            cut.push(at..at + chunk.len());
            at += chunk.len();
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(cut, split(&data, ChunkerConfig::SMALL));

    assert_eq!(vault.part_writer(id(1)).unwrap().finish().unwrap().part(), None);
    assert!(matches!(
        vault.part_writer(id(9)),
        Err(StoreError::MissingKey(_))
    ));
}
