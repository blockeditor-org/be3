use super::*;

#[test]
fn a_personal_organization_can_create_an_agent() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.agents");
    app.wait("Dialog \"Agents\"");
    app.crop("Dialog \"Agents\"");
    app.snapshot("a_personal_organization_can_manage_agents");
    app.click("#app.agents.name");
    app.type_text("Helper");
    app.click("#app.agents.add");
    app.wait("#app.agents.credentials");
    app.wait("#app.agents.copy");
    app.click("#app.agents.copy");
    let copied = app.run(&["clipboard"]);
    assert!(copied.trim().starts_with('{'));
    for key in ["server", "organization", "token", "recovery_phrase"] {
        assert!(copied.contains(&format!("\"{key}\":\"")));
    }
    app.run(&["clipboard", ""]);
    app.click("#app.agents.credentials");
    app.key("ctrl+a");
    app.key("ctrl+c");
    assert!(
        app.run(&["clipboard"]) == copied,
        "the text box can be copied manually"
    );
    app.crop("#app.agents.copy");
    app.snapshot("agent_credentials_have_a_copy_button");
    app.crop("#app.agents.credentials-help");
    app.snapshot("agent_credentials_are_copied_as_one_secret");
    app.close();
}
