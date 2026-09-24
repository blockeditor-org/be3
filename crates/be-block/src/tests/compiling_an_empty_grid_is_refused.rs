use logicgame::grid::LogicGrid as Grid;

use crate::compiled_logic::{CompileError, CompiledLogic};
use uuid::Uuid;

#[test]
fn compiling_an_empty_grid_is_refused() {
    let empty = Grid::new();

    assert_eq!(
        CompiledLogic::compile(Uuid::new_v4(), &empty),
        Err(CompileError::Empty)
    );
}
