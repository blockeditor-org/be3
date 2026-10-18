use super::*;

#[tokio::test]
async fn browser_approval_finishes_the_apps_login() {
    build_id::install("test");
    let directory = std::env::temp_dir().join(format!("block-app-browser-auth-{}", Uuid::new_v4()));
    let server = crate::platform::start_embedded_server(directory.clone()).unwrap();
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
    let page = ureq::get(&browser.url)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    let csrf = page
        .split("name=csrf value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    ureq::post(&auth)
        .set("Origin", &url)
        .send_form(&[
            ("csrf", csrf),
            ("code", &browser.code),
            ("action", "register"),
            ("email", "browser@example.com"),
            ("display_name", "Browser"),
            ("password", "correct horse battery"),
        ])
        .unwrap();
    let session = login.await.unwrap().unwrap();
    assert_eq!(session.email, "browser@example.com");
    assert_eq!(session.display_name, "Browser");
    assert!(!session.token.is_empty());
    drop(server);
    let _ = std::fs::remove_dir_all(directory);
}
