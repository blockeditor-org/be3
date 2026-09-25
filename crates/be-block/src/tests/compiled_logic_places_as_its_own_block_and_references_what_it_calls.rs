use crate::Root;
use crate::compiled_logic::{CompiledLogic, CompiledLogicDocument};
use logicgame::execution::{Instruction, UnlinkedComponent, UnlinkedSubgraph};
use logicgame::grid::{ComponentKind, ComponentPort, ComponentSide, Scale, Size};
use uuid::Uuid;

fn compiled(source: Uuid, calls: Vec<Uuid>) -> CompiledLogic {
    let instructions = vec![Instruction::Not {
        input: 0,
        output: 1,
    }];
    CompiledLogic::new(
        source,
        Size::new(2, 2),
        vec![
            ComponentPort::input(0, Scale::ONE, ComponentSide::Bottom, 0, 1),
            ComponentPort::output(0, Scale::ONE, ComponentSide::Top, 0, 1),
        ],
        UnlinkedComponent {
            inputs: vec![0],
            outputs: vec![1],
            components: calls,
            instructions: instructions.clone(),
            subgraphs: vec![UnlinkedSubgraph {
                inputs: vec![0],
                outputs: vec![0],
                instructions,
            }],
            memory_size: 2,
            storage_init: Vec::new(),
        },
    )
}

#[test]
fn compiled_logic_places_as_its_own_block_and_references_what_it_calls() {
    let id = Uuid::new_v4();
    let program = compiled(Uuid::new_v4(), Vec::new());

    let kind = program.placement(id, "Half Adder").unwrap();

    let ComponentKind::Subcomponent {
        compiled: called,
        name,
        size,
        ports,
        subgraphs,
        ..
    } = kind
    else {
        panic!("a compiled program places as a subcomponent");
    };
    assert_eq!(called, id);
    assert_eq!(name, "Half Adder");
    assert_eq!(size, program.size());
    assert_eq!(ports, program.ports());
    assert_eq!(subgraphs, program.subgraphs());

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let calling = CompiledLogicDocument::of(compiled(Uuid::new_v4(), vec![first, second]));
    assert_eq!(calling.references(), [first, second]);
}
