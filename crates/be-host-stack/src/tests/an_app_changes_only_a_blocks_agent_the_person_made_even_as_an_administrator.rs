use super::*;

/// An administrator's `remove-member` removes any member, so an app's `remove-agent` and
/// `rotate-agent-token` are refused for a person, or for an agent another member made.
#[test]
fn an_app_changes_only_a_blocks_agent_the_person_made_even_as_an_administrator() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let member = Someone::register(
        &harness.directory.join("server"),
        &url,
        "member@example.com",
        "Member",
    );
    harness.connect();
    let link = runtime()
        .block_on(crate::invite(
            "member@example.com".to_owned(),
            be_block_protocol::OrganizationRole::Editor,
        ))
        .expect("the administrator invites the member");
    let parsed = crate::invite_link::InviteLink::parse(&link).expect("the link parses");
    runtime()
        .block_on(crate::accounts::accept_invitation(
            url.clone(),
            member.session.token.clone(),
            member.me(),
            parsed.code,
        ))
        .expect("the member joins");
    let peer = runtime().block_on(member.peer(&url, harness.organization));
    let theirs = runtime()
        .block_on(be_block_client::ask(be_block_protocol::client::members::create_agent(
            &peer.connection(),
            harness.organization,
            "Theirs".into(),
            [7; 32],
            [7; 32],
            Vec::new(),
            Vec::new(),
            None,
        )))
        .expect("the member makes an agent")
        .agent
        .account;

    for target in [member.session.account, theirs] {
        let (reply, removed) = tokio::sync::oneshot::channel();
        crate::remove_block_agent(target, move |removed| {
            let _ = reply.send(removed);
        });
        let removed = runtime().block_on(removed).expect("the worker answers");
        assert!(removed.is_err(), "an app removed {target}: {removed:?}");
        let (reply, rotated) = tokio::sync::oneshot::channel();
        crate::rotate_agent_token(target, move |rotated| {
            let _ = reply.send(rotated);
        });
        let rotated = runtime().block_on(rotated).expect("the worker answers");
        assert!(rotated.is_err(), "an app rotated {target}'s token: {rotated:?}");
    }

    let (reply, listed) = tokio::sync::oneshot::channel();
    crate::list_members(Box::new(move |listed| {
        let _ = reply.send(listed);
    }));
    let members = runtime()
        .block_on(listed)
        .expect("the worker answers")
        .expect("the members list");
    for kept in [member.session.account, theirs] {
        assert!(
            members.iter().any(|listed| listed.account == kept),
            "{kept} is no longer a member: {members:?}"
        );
    }
}
