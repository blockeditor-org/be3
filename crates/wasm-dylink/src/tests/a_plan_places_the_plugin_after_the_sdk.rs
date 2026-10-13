use super::*;

#[test]
fn a_plan_places_the_plugin_after_the_sdk() {
    let declared = Declared {
        memory_size: 100,
        memory_align: 4,
        table_size: 3,
        table_align: 2,
    };
    let layout = plan(declared, 1001, 10, IMPORTS, &sdk(64)).unwrap();
    assert_eq!(layout.memory_base, 1008);
    assert_eq!(layout.table_base, 12);
    assert_eq!(layout.sdk_slots, vec![(15, "sdk_function".to_owned())]);
    assert_eq!(layout.plugin_slots, vec![(16, "own_function".to_owned())]);
    assert_eq!(layout.table_grow, 7);
    assert_eq!(
        layout.imports,
        vec![
            Import::Table,
            Import::Constant(1008),
            Import::Elsewhere,
            Import::Slot(15),
            Import::Slot(16),
            Import::Address(4096),
            Import::Own,
        ]
    );
    assert_eq!(layout.own_address("OWN_DATA", 8), Ok(1016));
}
