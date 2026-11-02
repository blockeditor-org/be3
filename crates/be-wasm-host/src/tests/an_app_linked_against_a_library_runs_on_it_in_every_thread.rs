use super::*;

#[test]
fn an_app_linked_against_a_library_runs_on_it_in_every_thread() {
    let identity = root_identity();
    let mut app = host()
        .load_bytes_with_libraries(linked_app(&identity).as_bytes(), |named| {
            assert_eq!(named, identity);
            Ok(root().as_bytes().to_vec())
        })
        .unwrap();
    app.start().unwrap();
    assert_eq!(settled(&mut app, 7 + 200 + 5), Ok(()));
}
