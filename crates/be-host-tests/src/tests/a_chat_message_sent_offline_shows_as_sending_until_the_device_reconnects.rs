use super::*;

/// A message sent while the device is offline stays on the device as sending, and goes once the
/// device is back.
#[test]
fn a_chat_message_sent_offline_shows_as_sending_until_the_device_reconnects() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Chat");
    phone.click("#chat.draft");
    phone.type_text("Hello");
    phone.click("#chat.send");
    phone.wait("\"Hello\"");
    phone.synced();
    assert!(
        !phone.tree().contains("Sending"),
        "a message the server took is not sending"
    );

    phone.offline();
    phone.click("#chat.draft");
    phone.type_text("Anyone there");
    phone.click("#chat.send");
    phone.wait("\"Anyone there\"");
    // The owner groups what it sends for `APPEND_INTERVAL` before it seals, on the stopped clock.
    phone.run(&["pause", "500"]);
    phone.wait("Sending");
    phone.snapshot("a_chat_message_sent_offline_shows_as_sending_until_the_device_reconnects");

    phone.online();
    phone.synced();
    phone.gone("Sending");
    assert!(
        !phone.tree().contains("Not sent"),
        "the message went once the device was back"
    );
    phone.close();
}
