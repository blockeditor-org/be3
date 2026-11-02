use super::*;

#[test]
fn a_request_goes_to_the_nearest_editor_around_it_that_takes_it() {
    let (desktop, organization, files, checklist) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let nesting = Nesting {
        shell: desktop,
        parents: HashMap::from([
            (organization, desktop),
            (files, organization),
            (checklist, organization),
        ]),
        accepting: vec![desktop, organization],
    };

    assert_eq!(
        nesting.route(Some(files)),
        organization,
        "the files inside an organization open blocks in that organization"
    );
    assert_eq!(nesting.route(Some(checklist)), organization);
    assert_eq!(
        nesting.route(Some(organization)),
        organization,
        "an editor that takes a request handles its own"
    );
}
