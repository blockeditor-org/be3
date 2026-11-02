use super::*;

#[test]
fn a_module_that_never_answers_runs_out_of_fuel_and_input_passes_through() {
    let mut remapper = hostile("(loop $forever (br $forever)) (unreachable)");

    assert_eq!(remapper.input(press("A"), NOW), vec![press("A")]);
    assert_eq!(
        remapper.take_problems(),
        vec![
            "The input remaps failed (they ran out of fuel) and start again with the next key or button."
        ]
    );
    assert_eq!(remapper.input(release("A"), NOW), vec![release("A")]);
}
