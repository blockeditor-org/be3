use super::*;

const ASSETS: &[be_portal::Asset] = &[be_portal::Asset {
    name: "start.js",
    content_type: "text/javascript",
    bytes: b"export {};\n",
}];

#[tokio::test]
async fn the_portal_serves_its_assets_immutably_beside_a_hydrating_page() {
    let harness = Harness::configured(ServerConfig {
        portal: Some(be_portal::Portal {
            assets: ASSETS,
            ..be_portal_ui::PORTAL
        }),
        ..ServerConfig::OPEN
    })
    .await;
    let (status, page) = http(&harness, "GET", "/auth/login", "", None).await;
    assert_eq!(status, 200);
    for header in [
        "Cross-Origin-Opener-Policy: same-origin",
        "Cross-Origin-Embedder-Policy: require-corp",
        "script-src 'self' 'wasm-unsafe-eval' 'sha256-",
        "form-action 'self'",
        "Cache-Control: no-store",
    ] {
        assert!(page.contains(header), "{header}");
    }
    let script = page
        .split("<script type=\"module\" src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    assert!(script.starts_with("/auth/assets/") && script.ends_with("/start.js"));
    let (status, asset) = http(&harness, "GET", &script, "", None).await;
    assert_eq!(status, 200);
    assert!(asset.contains("Cache-Control: public, max-age=31536000, immutable"));
    assert!(asset.contains("Content-Type: text/javascript"));
    assert!(asset.ends_with("export {};\n"));
    let (status, _) = http(
        &harness,
        "GET",
        "/auth/assets/0000000000000000/start.js",
        "",
        None,
    )
    .await;
    assert_eq!(status, 404);
}
