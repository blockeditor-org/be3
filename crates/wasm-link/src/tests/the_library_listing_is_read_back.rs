use super::*;

#[test]
fn the_library_listing_is_read_back() {
    let found = map::thread_locals(MAP).unwrap();
    let listing = listing(
        "beui",
        &["cafe"],
        &["libbeui-1.rlib".to_owned(), "-lc".to_owned()],
        &found,
    );
    let module = module::with_custom(b"\0asm\x01\0\0\0", LIBRARY_SECTION, listing.as_bytes());
    let read = Library::parse(&module).unwrap();
    assert_eq!(read.name, "beui");
    assert_eq!(read.needs, ["cafe"]);
    assert_eq!(read.linked, ["libbeui-1.rlib", "-lc"]);
    assert_eq!(
        read.thread_locals,
        HashSet::from(["LOCAL".to_owned(), "errno".to_owned()])
    );
    assert_eq!(read.identity, wasm_dylink::identity(&module));
}
