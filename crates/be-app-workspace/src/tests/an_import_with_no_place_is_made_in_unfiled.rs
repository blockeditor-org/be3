use super::*;

#[test]
fn an_import_with_no_place_is_made_in_unfiled() {
    let (fixture, _) = editor();
    let mut fixture = picking(&fixture, filter(), BlockLocation::Unfiled);
    fixture.settle();
    fixture.test.take_requests();

    fixture.test.click("picker.import");
    fixture.settle();
    let place = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ImportBlocks { parent, .. } => Some(parent),
            _ => None,
        })
        .expect("choosing a file asks the host to import one");
    assert_eq!(
        place,
        BlockLocation::Unfiled,
        "a file imported with no place chosen goes to Unfiled"
    );
}
