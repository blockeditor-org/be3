use super::*;

#[test]
fn a_plan_places_every_module_after_the_ones_it_needs() {
    let modules = diamond();
    assert_eq!(pages(&modules), Ok(1));
    let plan = plan(&modules, 65536, 10).unwrap();
    let [root, left, plugin] = &plan.modules[..] else {
        panic!("{plan:?}");
    };
    assert_eq!((root.memory_base, root.imports.len()), (0, 0));
    assert_eq!(
        (left.memory_base, left.table_base, left.block),
        (65536, 12, 0)
    );
    assert_eq!(
        (plugin.memory_base, plugin.table_base, plugin.block),
        (65636, 15, 16)
    );
    assert_eq!(plan.region, 65648);
    assert_eq!(
        plan.blocks,
        Blocks {
            size: 40,
            align: 8,
            pages: 1
        }
    );
    assert_eq!(
        left.imports,
        vec![
            Import::Table,
            Import::Constant(65536),
            Import::Function(0),
            Import::ThreadLocal(0),
        ]
    );
    assert_eq!(
        plugin.imports,
        vec![
            Import::Function(1),
            Import::FunctionSlot(16),
            Import::FunctionSlot(17),
            Import::Address(0),
            Import::Address(1),
            Import::Address(2),
            Import::ThreadLocal(1),
            Import::Elsewhere,
        ]
    );
    assert_eq!(
        plan.slots,
        vec![
            Slot {
                slot: 16,
                module: 0,
                name: "malloc".to_owned()
            },
            Slot {
                slot: 17,
                module: 2,
                name: "own_function".to_owned()
            },
        ]
    );
    assert_eq!(plan.table_grow, 8);
    assert_eq!(plan.address(2, 8), Ok(65644));
    assert_eq!(plan.block(1000, 2), 1016);
}
