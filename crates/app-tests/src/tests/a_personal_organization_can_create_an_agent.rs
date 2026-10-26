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
    app.wait("Button \"Copy Agent credentials\"");
    app.wait("Dialog \"Agent credentials\"");
    let tree = app.tree();
    assert!(!tree.contains("TextInput \"Agent name\""));
    assert!(!tree.contains("Button \"Add agent\""));
    app.click("Button \"Copy Agent credentials\"");
    let copied = app.run(&["screen", "clipboard"]);
    assert!(copied.trim().starts_with('{'));
    for key in ["server", "organization", "token", "recovery_phrase"] {
        assert!(copied.contains(&format!("\"{key}\":\"")));
    }
    app.run(&["screen", "clipboard", ""]);
    app.click("#app.agents.credentials");
    app.key("ctrl+a");
    app.key("ctrl+c");
    assert!(
        app.run(&["screen", "clipboard"]) == copied,
        "the text box can be copied manually"
    );
    app.key("ctrl+a");
    app.type_text(r#"{"server":"https://blocks.example","organization":"00000000-0000-0000-0000-000000000001","token":"example-agent-token","recovery_phrase":"example agent recovery phrase"}"#);
    app.key("Home");
    app.click("#app.agents.credentials-help");
    app.crop("Dialog \"Agent credentials\"");
    app.snapshot("agent_credentials_are_copied_as_one_secret");
    app.click("#app.agents.proxy");
    app.click("Button \"Copy Agent credentials\"");
    let proxy = app.run(&["screen", "clipboard"]);
    assert!(proxy.contains("\"authentication\":\"proxy\""));
    assert!(!proxy.contains("\"token\":"));
    app.click("#app.agents.header");
    app.key("ctrl+a");
    app.type_text("Bearer example-agent-token");
    app.key("Home");
    app.click("#app.agents.credentials");
    app.key("ctrl+a");
    app.type_text(r#"{"server":"https://blocks.example","organization":"00000000-0000-0000-0000-000000000001","authentication":"proxy","recovery_phrase":"example agent recovery phrase"}"#);
    app.key("Home");
    app.click("#app.agents.credentials-help");
    app.crop("Dialog \"Agent credentials\"");
    app.snapshot("agent_account_token_is_supplied_by_proxy");
    app.click("#app.agents.cancel");
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.agents");
    app.wait("#app.agents.name");
    let tree = app.tree();
    assert!(tree.contains("Helper"));
    assert!(!tree.contains("TextInput \"Agent credentials\""));
    assert!(!tree.contains("Copy Agent credentials"));
    app.close();
}
