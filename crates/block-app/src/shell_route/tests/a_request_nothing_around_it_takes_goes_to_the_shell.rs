use super::*;

#[test]
fn a_request_nothing_around_it_takes_goes_to_the_shell() {
    let (desktop, note, looped) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let nesting = Nesting {
        shell: desktop,
        parents: HashMap::from([(note, desktop), (looped, looped)]),
        accepting: Vec::new(),
    };

    assert_eq!(nesting.route(None), desktop, "the host's own requests");
    assert_eq!(nesting.route(Some(note)), desktop);
    assert_eq!(nesting.route(Some(desktop)), desktop);
    assert_eq!(
        nesting.route(Some(looped)),
        desktop,
        "a chain that loops back on itself ends at the shell"
    );
}
