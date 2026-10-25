use super::*;

#[test]
fn a_message_waits_for_the_instance_it_names_to_be_opened() {
    let mut instances = placed();
    let presence = Outbound::Editor(INSTANCE, EditorCall::Presence(true));

    assert!(instances.gate(vec![presence.clone()]).is_empty());

    let opened = instances.next_screens(PASS).opened;
    let open = opened.iter().position(|message| {
        matches!(message, Outbound::Open(_))
    });
    let held = opened.iter().position(|message| message == &presence);

    assert!(open.is_some());
    assert_eq!(open.zip(held).map(|(open, held)| open < held), Some(true));
    assert_eq!(instances.gate(vec![presence.clone()]), vec![presence]);
}
