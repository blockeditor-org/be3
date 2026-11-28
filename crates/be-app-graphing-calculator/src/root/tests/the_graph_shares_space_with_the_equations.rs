use super::*;

#[test]
fn the_graph_shares_space_with_the_equations() {
    let mut test = calculator(vec2(1000.0, 640.0));
    let graph = test.rect_of("graphing-calculator.stage");
    let add = test.rect_of("graphing-calculator.add");
    assert!(add.right() <= graph.left());
    test.snapshot("graphing_calculator_on_desktop");

    test.set_size(vec2(390.0, 844.0));
    test.settle();
    let graph = test.rect_of("graphing-calculator.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(graph.bottom() <= gathered.top() + 1.0);
    test.snapshot("graphing_calculator_on_a_phone");
}
