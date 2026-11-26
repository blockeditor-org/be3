use super::*;
#[tokio::test]
/// The host begins a sign-in and polls it with one-shot calls, holding no connection while the
/// person approves it in the browser, and connects with the token once approved.
async fn browser_approval_finishes_the_hosts_login() {
    be_lib_build_id::install("test");
    let directory = std::env::temp_dir().join(format!("be-host-browser-auth-{}", Uuid::new_v4()));
    let store = be_server::ServerStore::open(&directory).unwrap();
    let (profile, _) = store
        .register("browser@example.com", "Browser")
        .await
        .unwrap();
    // The person is signed in to the portal already; how is not what this test is about.
    let cookie = format!(
        "be3-account={}",
        store.browser_session(profile.account).await.unwrap()
    );
    let cookie = cookie.as_str();
    let server = be_host_server::start_embedded_server(directory.clone()).unwrap();
    let url = server.url.clone();
    let auth = format!("{url}/auth");
    let (notify, notified) = tokio::sync::oneshot::channel();
    let (check_now, checks) = tokio::sync::mpsc::unbounded_channel();
    let login = tokio::spawn(browser_login(
        url.clone(),
        auth.clone(),
        move |browser| {
            let _ = notify.send(browser);
        },
        checks,
        Box::pin(std::future::pending()),
    ));
    let browser = notified.await.unwrap();
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let csrf = |page: &str| {
        page.split("\"csrf\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string()
    };
    let page = agent
        .get(&browser.url)
        .set("Cookie", cookie)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    agent
        .post(&format!("{auth}/approve"))
        .set("Origin", &url)
        .set("Cookie", cookie)
        .send_form(&[("csrf", &csrf(&page)), ("code", &browser.code)])
        .unwrap();
    // Checking now polls as soon as the server allows, rather than at the next turn of
    // `LOGIN_POLL_EVERY`.
    check_now.send(()).unwrap();
    let session = tokio::time::timeout(LOGIN_POLL_EVERY, login)
        .await
        .expect("checking now did not poll before the next turn")
        .unwrap()
        .unwrap();
    assert_eq!(session.email, "browser@example.com");
    assert_eq!(session.display_name, "Browser");
    assert!(!session.token.is_empty());
    drop(server);
    let _ = std::fs::remove_dir_all(directory);
}
