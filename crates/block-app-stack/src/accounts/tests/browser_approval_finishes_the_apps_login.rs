use super::*;
#[tokio::test]
async fn browser_approval_finishes_the_apps_login() {
    build_id::install("test");
    let directory = std::env::temp_dir().join(format!("block-app-browser-auth-{}", Uuid::new_v4()));
    let (_, _) = be_server::ServerStore::open(&directory)
        .unwrap()
        .register("browser@example.com", "Browser", "correct horse battery")
        .await
        .unwrap();
    let server = crate::embedded_server::start_embedded_server(directory.clone()).unwrap();
    let url = server.url.clone();
    let auth = format!("{url}/auth");
    let (notify, notified) = tokio::sync::oneshot::channel();
    let login = tokio::spawn(browser_login(
        url.clone(),
        auth.clone(),
        move |browser| {
            let _ = notify.send(browser);
        },
        Box::pin(std::future::pending()),
    ));
    let browser = notified.await.unwrap();
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let page = agent
        .get(&browser.url)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    let csrf = |page: &str| {
        page.split("\"csrf\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string()
    };
    let signed_in = agent
        .post(&format!("{auth}/login"))
        .set("Origin", &url)
        .send_form(&[
            ("csrf", &csrf(&page)),
            ("code", &browser.code),
            ("email", "browser@example.com"),
            ("password", "correct horse battery"),
        ])
        .unwrap();
    assert_eq!(signed_in.status(), 303);
    let cookie = signed_in
        .header("Set-Cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap();
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
    let session = login.await.unwrap().unwrap();
    assert_eq!(session.email, "browser@example.com");
    assert_eq!(session.display_name, "Browser");
    assert!(!session.token.is_empty());
    drop(server);
    let _ = std::fs::remove_dir_all(directory);
}
