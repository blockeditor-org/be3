use super::*;

#[test]
fn a_personal_organization_can_create_an_agent() {
    let mut host = HostTest::start();
    host.click("#file-tree.host-menu");
    host.click("#host.menu.agents");
    host.wait("Dialog \"Agents\"");
    host.crop("Dialog \"Agents\"");
    host.snapshot("a_personal_organization_can_manage_agents");
    host.click("#host.agents.name");
    host.type_text("Helper");
    host.click("#host.agents.add");
    host.wait("#host.agents.credentials");
    host.wait("Button \"Copy Agent credentials\"");
    host.wait("Dialog \"Agent credentials\"");
    let tree = host.tree();
    assert!(!tree.contains("TextInput \"Agent name\""));
    assert!(!tree.contains("Button \"Add agent\""));
    host.click("Button \"Copy Agent credentials\"");
    let copied = host.run(&["clipboard"]);
    assert!(copied.trim().starts_with('{'));
    for key in ["server", "organization", "token", "recovery_phrase"] {
        assert!(copied.contains(&format!("\"{key}\":\"")));
    }
    host.run(&["clipboard", ""]);
    host.click("#host.agents.credentials");
    host.key("ctrl+a");
    host.key("ctrl+c");
    assert!(
        host.run(&["clipboard"]) == copied,
        "the text box can be copied manually"
    );
    host.key("ctrl+a");
    host.type_text(r#"{"server":"https://blocks.example","organization":"00000000-0000-0000-0000-000000000001","token":"example-agent-token","recovery_phrase":"example agent recovery phrase"}"#);
    host.key("Home");
    host.click("#host.agents.credentials-help");
    host.crop("Dialog \"Agent credentials\"");
    host.snapshot("agent_credentials_are_copied_as_one_secret");
    host.click("#host.agents.proxy");
    host.click("Button \"Copy Agent credentials\"");
    let proxy = host.run(&["clipboard"]);
    assert!(proxy.contains("\"authentication\":\"proxy\""));
    assert!(!proxy.contains("\"token\":"));
    host.click("#host.agents.header");
    host.key("ctrl+a");
    host.type_text("Bearer example-agent-token");
    host.key("Home");
    host.click("#host.agents.credentials");
    host.key("ctrl+a");
    host.type_text(r#"{"server":"https://blocks.example","organization":"00000000-0000-0000-0000-000000000001","authentication":"proxy","recovery_phrase":"example agent recovery phrase"}"#);
    host.key("Home");
    host.click("#host.agents.credentials-help");
    host.crop("Dialog \"Agent credentials\"");
    host.snapshot("agent_account_token_is_supplied_by_proxy");
    host.click("#host.agents.cancel");
    host.click("#file-tree.host-menu");
    host.click("#host.menu.agents");
    host.wait("#host.agents.name");
    let tree = host.tree();
    assert!(tree.contains("Helper"));
    assert!(!tree.contains("TextInput \"Agent credentials\""));
    assert!(!tree.contains("Copy Agent credentials"));
    host.close();
}
