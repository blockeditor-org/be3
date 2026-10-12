use super::*;

#[test]
fn the_sdk_listing_is_read_back() {
    let mut found = map::thread_locals(MAP).unwrap();
    found.symbols.push(map::Placed {
        name: PLUGIN_TLS.to_owned(),
        offset: 16,
        size: PLUGIN_TLS_SIZE,
    });
    let listing = sdk_listing(&["libbeui-1.rlib".to_owned(), "-lc".to_owned()], &found).unwrap();
    let module = module::with_custom(b"\0asm\x01\0\0\0", SDK_SECTION, listing.as_bytes());
    let read = Sdk::parse(&module).unwrap();
    assert_eq!(read.linked, HashSet::from(["libbeui-1.rlib".to_owned(), "-lc".to_owned()]));
    assert_eq!(read.reserve, (16, PLUGIN_TLS_SIZE));
    assert_eq!(
        read.thread_locals
            .iter()
            .map(|local| (local.name.as_str(), local.offset))
            .collect::<Vec<_>>(),
        [("LOCAL", 0), ("errno", 4)]
    );
    assert_eq!(read.identity, identity(&module));
    assert_eq!(module::sections(&read.mirror()).unwrap().len(), 3);
}
