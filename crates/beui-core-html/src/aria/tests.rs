use super::*;
use crate::document::{create, element, set_accessibility};

fn html(tag: &'static str, node: accesskit::Node) -> String {
    let made = create("Test", tag);
    set_accessibility(made, node);
    element(made).expect("made above").to_html()
}

fn with(role: Role, set: impl FnOnce(&mut accesskit::Node)) -> accesskit::Node {
    let mut node = accesskit::Node::new(role);
    set(&mut node);
    node
}

mod a_changed_role_drops_the_attributes_it_no_longer_has;
mod a_checked_switch_reads_as_a_checked_aria_switch;
mod a_native_element_keeps_its_own_semantics;
mod a_selected_tab_reads_as_a_selected_aria_tab;
mod a_slider_carries_its_value_and_range;
