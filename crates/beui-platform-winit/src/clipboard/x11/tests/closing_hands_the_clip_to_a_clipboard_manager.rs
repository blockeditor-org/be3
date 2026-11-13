use super::*;

#[test]
fn closing_hands_the_clip_to_a_clipboard_manager() {
    let Some(server) = Xvfb::start() else {
        return;
    };
    let _watchdog = Watchdog::start();
    let owner = Owner::connect(Some(&server.display)).expect("the owner connects");
    owner.offer(Some("kept"), None);

    let (ready, managing) = std::sync::mpsc::channel();
    let display = server.display.clone();
    let manager = std::thread::spawn(move || {
        let manager = Reader::connect(&display);
        manager
            .connection
            .set_selection_owner(
                manager.window,
                manager.atom("CLIPBOARD_MANAGER"),
                CURRENT_TIME,
            )
            .expect("the manager takes its selection");
        manager
            .connection
            .get_input_focus()
            .expect("sync")
            .reply()
            .expect("synced");
        ready.send(()).expect("the test waits");
        let request = loop {
            if let Event::SelectionRequest(request) =
                manager.connection.wait_for_event().expect("an event")
            {
                break request;
            }
        };
        assert_eq!(request.target, manager.atom("SAVE_TARGETS"));
        let saved = manager.read("UTF8_STRING");
        manager
            .connection
            .send_event(
                false,
                request.requestor,
                EventMask::NO_EVENT,
                SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: request.time,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property: request.property,
                },
            )
            .expect("the manager answers");
        manager.connection.flush().expect("flush");
        saved
    });
    managing.recv().expect("the manager is ready");

    drop(owner);
    assert_eq!(
        manager.join().expect("the manager finishes").as_deref(),
        Some(&b"kept"[..]),
        "the manager read the clip before the owner went"
    );
}
