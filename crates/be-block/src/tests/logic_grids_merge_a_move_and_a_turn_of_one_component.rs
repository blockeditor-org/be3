use crate::Merge;
use crate::logic_grid::{LogicGridContent, LogicGridOperation};
use be_commit::MergeResult;
use logicgame::grid::{Component, ComponentId, ComponentKind, ComponentOrientation, Point, Scale};

fn not_gate(id: u64) -> Component {
    Component {
        id: ComponentId(id),
        position: Point::new(0, 0),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Not { scale: Scale::ONE },
    }
}

fn run(content: &LogicGridContent, operation: LogicGridOperation) -> LogicGridContent {
    let mut changed = content.clone();
    let edit = content.root().edit_for(&operation);
    changed.apply(&edit);
    changed
}

#[test]
fn logic_grids_merge_a_move_and_a_turn_of_one_component() {
    let base = run(
        &LogicGridContent::default(),
        LogicGridOperation::AddComponent {
            component: not_gate(0),
        },
    );
    let ours = run(
        &base,
        LogicGridOperation::MoveComponent {
            id: ComponentId(0),
            position: Point::new(8, 0),
        },
    );
    let theirs = run(
        &base,
        LogicGridOperation::OrientComponent {
            id: ComponentId(0),
            orientation: ComponentOrientation::Right,
        },
    );

    let MergeResult::Clean(merged) = LogicGridContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different fields of the component");
    };

    let grid = merged.root().grid();
    let component = grid.component(ComponentId(0)).unwrap();
    assert_eq!(component.position, Point::new(8, 0));
    assert_eq!(component.orientation, ComponentOrientation::Right);
}
