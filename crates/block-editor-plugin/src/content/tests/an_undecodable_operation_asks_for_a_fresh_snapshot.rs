use super::*;

#[test]
fn an_undecodable_operation_asks_for_a_fresh_snapshot() {
    let fixture = Fixture::new();
    let rename = Checklist::set_text(fixture.second, "free range eggs");
    fixture.host.push_content_operations(vec![
        (vec![0xff; 3], false),
        (ChecklistContent::encode_operation(&rename), false),
    ]);
    fixture.projection.pump();

    assert_eq!(fixture.host.take_content_resend_requests(), [None]);
    assert_eq!(fixture.text(fixture.second), "eggs");

    fixture.arrive(&[(Checklist::set_text(fixture.first, "oat milk"), false)]);
    assert_eq!(fixture.text(fixture.first), "milk");

    let mut content = fixture.projection.read(|content| content.clone()).unwrap();
    content.apply(&rename);
    fixture
        .host
        .set_block_content(ChecklistContent::CONTENT_TYPE, content.encode(), 0);
    fixture.projection.pump();

    assert_eq!(fixture.text(fixture.second), "free range eggs");
    assert_eq!(fixture.text(fixture.first), "milk");
}
