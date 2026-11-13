use super::*;

#[test]
fn a_be3_format_crosses_to_another_program_with_its_refs() {
    let Some(server) = Xvfb::start() else {
        return;
    };
    let _watchdog = Watchdog::start(&server);
    let copying = Owner::connect(Some(&server.display)).expect("the copying program connects");
    let pasting = Owner::connect(Some(&server.display)).expect("the pasting program connects");
    let mut blocks = ClipItem::new(formats::BLOCKS, b"blocks".to_vec());
    blocks.refs = vec![Default::default()];
    // Larger than one piece, so it goes by INCR.
    let mut large = ClipItem::new(formats::CANVAS_ITEMS, vec![3; CHUNK * 2]);
    large.refs = vec![Default::default(), Default::default()];
    copying.offer(&[ClipItem::text("plain"), blocks.clone(), large.clone()]);

    let reader = Reader::connect(&server.display);
    let targets = reader.targets();
    assert!(
        targets.iter().any(|target| target == formats::BLOCKS),
        "be3/blocks in {targets:?}"
    );
    assert_eq!(pasting.read(formats::BLOCKS), Some(blocks), "the item keeps its refs");
    assert_eq!(pasting.read(formats::CANVAS_ITEMS), Some(large));
    assert_eq!(pasting.read(formats::FILES), None, "a format not offered is not read");
}
