use super::*;

#[test]
fn creating_a_triangle_makes_a_block_with_no_state() {
    let host = EditorHost::default();
    let mut triangle = Triangle::new(host.clone());

    let created = triangle
        .create_block()
        .expect("a triangle can always be made");

    let commands = host.take_graph_commands();
    let [GraphCommand::Create { id, block_type, .. }] = commands.as_slice() else {
        panic!(
            "expected one block to be created, got {} commands",
            commands.len()
        );
    };
    assert_eq!(*id, created);
    assert_eq!(*block_type, TriangleContent::CONTENT_TYPE);
}
