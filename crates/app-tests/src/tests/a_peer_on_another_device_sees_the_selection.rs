use super::*;

#[test]
fn a_peer_on_another_device_sees_the_selection() {
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port is found")
        .to_string();
    let mut here = AppTest::start_with(&[&format!("--server-address={address}")], "Pane \"Files\"");
    add_root_block(&mut here, "Text");
    here.click("#text.surface");
    here.type_text("Seen from two devices");
    here.wait("TreeItem \"Seen from two devices\"");
    here.key("Home");
    here.key("shift+End");
    here.run(&["settle"]);

    let mut there = AppTest::start_signed_out();
    there.click("Button \"Add account\"");
    there.click("Tab \"Remote\"");
    there.click("TextInput \"Server address\"");
    there.type_text(&format!("http://{address}"));
    there.click("TextInput \"Email address\"");
    there.type_text("dev@localhost");
    there.click("TextInput \"Password\"");
    there.type_text("dev-password");
    there.click("Button \"Log in\"");
    there.click("Button \"Dev\"");
    there.click("Button \"Show a code\"");
    there.wait("Waiting for the other device");
    let code = pairing_code(&mut there);
    here.click("TextInput \"Code\"");
    here.type_text(&code);
    here.click("Button \"Approve\"");
    there.click("TreeItem \"Seen from two devices\"");
    there.wait("Pane \"Text\"");
    there.run(&["settle"]);

    here.key("shift+Left");
    here.run(&["settle"]);
    there.click("#text.surface");
    there.key("End");
    there.crop("#text.surface");
    there.snapshot("a_peer_on_another_device_sees_the_selection");
    there.close();
    here.close();
}

fn pairing_code(app: &mut AppTest) -> String {
    app.tree()
        .lines()
        .filter_map(|line| line.split("value=\"").nth(1)?.split('"').next())
        .find(|value| {
            value.len() == 9
                && value
                    .chars()
                    .enumerate()
                    .all(|(index, character)| match index {
                        4 => character == '-',
                        _ => character.is_ascii_alphanumeric(),
                    })
        })
        .unwrap_or_else(|| panic!("no pairing code is shown"))
        .to_owned()
}
