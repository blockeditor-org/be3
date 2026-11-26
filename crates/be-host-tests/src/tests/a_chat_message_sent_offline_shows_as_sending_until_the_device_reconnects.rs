use super::*;

/// A message sent while the device is offline stays on the device as sending, and goes once the
/// device is back.
#[test]
fn a_chat_message_sent_offline_shows_as_sending_until_the_device_reconnects() {
    let server = server();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Chat");
    phone.click("#chat.draft");
    phone.type_text("Hello");
    phone.click("#chat.send");
    phone.wait("\"Hello\"");
    phone.synced();
    // A message the server took is not sending.
    phone.gone("Sending");

    phone.offline();
    phone.click("#chat.draft");
    phone.type_text("Anyone there");
    phone.click("#chat.send");
    phone.wait("\"Anyone there\"");
    // Offline, the device numbers the message itself; it still shows as sending a while on, on
    // the stopped clock.
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
