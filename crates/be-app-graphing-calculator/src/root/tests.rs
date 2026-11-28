use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use beui::{Vec2, vec2};
use uuid::Uuid;

use super::{GraphRoot, initial_graph};

mod the_graph_shares_space_with_the_equations;

fn calculator(size: Vec2) -> BeuiTest<GraphRoot> {
    let mut test = BeuiTest::editing(Uuid::new_v4()).in_viewport().with_size(size);
    test.store().register(be_type_graphing_calculator::registrations());
    test.hold(None, initial_graph());
    test.settle();
    test
}
