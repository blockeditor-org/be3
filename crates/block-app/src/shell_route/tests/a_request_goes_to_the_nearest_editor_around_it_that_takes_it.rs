use super::*;

#[test]
fn a_request_goes_to_the_nearest_editor_around_it_that_takes_it() {
    let (desktop, workspace, files, checklist) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let nesting = Nesting {
        shell: desktop,
        parents: HashMap::from([
            (workspace, desktop),
            (files, workspace),
            (checklist, workspace),
        ]),
        accepting: vec![desktop, workspace],
    };

    assert_eq!(
        nesting.route(Some(files)),
        workspace,
        "the files inside a workspace open blocks in that workspace"
    );
    assert_eq!(nesting.route(Some(checklist)), workspace);
    assert_eq!(
        nesting.route(Some(workspace)),
        workspace,
        "an editor that takes a request handles its own"
    );
}
