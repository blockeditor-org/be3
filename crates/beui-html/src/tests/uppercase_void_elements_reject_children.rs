use super::*;

#[test]
#[should_panic(expected = "void HTML elements cannot have children")]
fn uppercase_void_elements_reject_children() {
    Node::element("INPUT").child("bad");
}
