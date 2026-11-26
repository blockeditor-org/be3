use super::*;

#[test]
fn an_app_linked_against_a_library_runs_on_it() {
    let identity = root_identity();
    let mut app = host()
        .load_bytes_with_libraries(linked_app(&identity).as_bytes(), |named| {
            assert_eq!(named, identity);
            Ok(root().as_bytes().to_vec())
        })
        .unwrap();
    app.start().unwrap();
    app.step().unwrap();

    assert_eq!(app.take_outbound(), vec![vec![42 + 5]]);
}
