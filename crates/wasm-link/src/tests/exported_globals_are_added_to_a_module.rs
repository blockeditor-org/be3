use super::*;

#[test]
fn exported_globals_are_added_to_a_module() {
    let empty = b"\0asm\x01\0\0\0".to_vec();
    let described = module::with_exported_globals(
        &empty,
        &[("first".to_owned(), 7), ("second".to_owned(), -1)],
    )
    .unwrap();
    let exports = module::exports(&described).unwrap();
    assert_eq!(
        exports,
        [
            module::Export {
                name: "first".to_owned(),
                kind: module::GLOBAL_KIND
            },
            module::Export {
                name: "second".to_owned(),
                kind: module::GLOBAL_KIND
            }
        ]
    );
    let again = module::with_exported_globals(&described, &[("third".to_owned(), 1)]).unwrap();
    assert_eq!(module::exports(&again).unwrap().len(), 3);
    assert_eq!(module::exported_constant(&again, "second"), Ok(Some(-1)));
    assert_eq!(module::exported_constant(&again, "third"), Ok(Some(1)));
    assert_eq!(module::exported_constant(&again, "fourth"), Ok(None));
}
