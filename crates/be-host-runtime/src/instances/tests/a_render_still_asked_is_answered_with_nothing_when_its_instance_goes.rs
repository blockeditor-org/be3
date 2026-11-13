use super::*;

fn unanswered(heard: &[InstanceEvent], asked: u64) -> bool {
    heard.iter().any(|event| {
        matches!(event, InstanceEvent::Rendered { request, body: None } if *request == asked)
    })
}

#[test]
fn a_render_still_asked_is_answered_with_nothing_when_its_instance_goes() {
    let (_mailbox, heard) = hearing();

    let mut instances = placed();
    let (_, asked) = instances.render_clip(INSTANCE, 1, "text/plain".to_owned());
    let asked = asked.expect("the instance is open");
    instances.remove(INSTANCE);
    host::deliver();
    assert!(
        unanswered(&heard.borrow(), asked),
        "a closed instance answers the paste with nothing"
    );
    assert!(
        heard
            .borrow()
            .iter()
            .any(|event| matches!(event, InstanceEvent::Closed)),
        "and says it closed"
    );
    heard.borrow_mut().clear();

    let mut instances = placed();
    let (_, asked) = instances.render_clip(INSTANCE, 1, "text/plain".to_owned());
    let asked = asked.expect("the instance is open");
    instances.reopen();
    host::deliver();
    assert!(
        unanswered(&heard.borrow(), asked),
        "an app that restarts forgets what it was asked to render"
    );
    heard.borrow_mut().clear();

    let mut instances = placed();
    let (_, asked) = instances.render_clip(INSTANCE, 1, "text/plain".to_owned());
    let asked = asked.expect("the instance is open");
    drop(instances);
    host::deliver();
    assert!(
        unanswered(&heard.borrow(), asked),
        "instances that go with their app answer with nothing"
    );
}
