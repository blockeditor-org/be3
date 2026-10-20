use super::*;

fn clear_formatting(transform: &mut Transform<'_>) {
    let Some(formats) = transform.formats() else {
        return;
    };
    for range in transform.selections().to_vec() {
        let keys: Vec<Name> = formats
            .marks_in(range.clone())
            .flat_map(|(_, marks)| marks.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>())
            .collect();
        for key in keys {
            transform.set_mark(range.clone(), &key, None);
        }
    }
}

#[test]
fn rich_text_a_transform_composes_generic_operations() {
    let mut tester = EditorTester::rich(b"loud and clear");
    select(&mut tester, 0..8);
    tool(&mut tester, "bold");
    tool(&mut tester, "italic");
    assert_eq!(formatted(&tester), "{bold,italic}loud and{} clear");

    select(&mut tester, 2..14);
    tester.execute(EditorCommand::Transform(clear_formatting));
    assert_eq!(formatted(&tester), "{bold,italic}lo{}ud and clear");
    tester.execute(EditorCommand::Undo);
    assert_eq!(
        formatted(&tester),
        "{bold,italic}loud and{} clear",
        "a transform undoes as one step"
    );
}
