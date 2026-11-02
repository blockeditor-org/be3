use super::*;

#[derive(Default)]
struct Owner {
    heard: Vec<String>,
}

#[test]
fn a_desk_hands_its_owner_to_the_handlers_and_a_dropped_mailbox_hears_nothing() {
    let desk = Desk::<Owner>::default();
    let (_kept, kept) = desk.mailbox(|owner: &mut Owner, word: &'static str| {
        owner.heard.push(word.to_owned());
    });
    let (dropped, gone) = desk.mailbox(|owner: &mut Owner, number: u32| {
        owner.heard.push(number.to_string());
    });
    let framed = Rc::new(Cell::new(0));
    let counted = Rc::clone(&framed);
    let (_frame, frame) = mailbox(move |added: u32| counted.set(counted.get() + added));

    let worker = {
        let kept = kept.clone();
        thread::spawn(move || kept.send("from a worker"))
    };
    worker.join().unwrap();
    kept.send("here");
    gone.send(7);
    frame.send(2);
    drop(dropped);

    deliver();
    assert_eq!(
        framed.get(),
        2,
        "the frame's mailboxes are delivered at its start"
    );

    let mut owner = Owner::default();
    desk.deliver(&mut owner);
    assert_eq!(
        owner.heard,
        vec!["from a worker".to_owned(), "here".to_owned()],
        "letters arrive in the order they were posted, and none for a dropped mailbox"
    );

    desk.deliver(&mut owner);
    assert_eq!(owner.heard.len(), 2, "each letter is delivered once");
}
