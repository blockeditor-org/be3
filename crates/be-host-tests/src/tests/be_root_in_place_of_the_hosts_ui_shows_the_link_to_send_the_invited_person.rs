use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_the_link_to_send_the_invited_person() {
    // be-host's own UI makes an organization; started again with the root in place of its UI,
    // the root's Invite member makes the link at once and shows it, as text and as a QR code,
    // as be-host's does (an_invitation_shows_the_link_to_send_the_invited_person).
    let server = server();
    let dev = server.account("Dev");
    let mut host = HostTest::drawn_device(&dev);
    create_organization(&mut host, "Team");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.invite");
    host.wait("#host.invite.link");
    host.wait("#host.invite.qr");
    host.crop("#host.invite.help");
    host.snapshot("be_root_shows_the_link_to_send_the_invited_person");
    host.close();
}
