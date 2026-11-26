use super::*;

#[test]
fn the_cdn_must_send_isolation_and_worker_headers() {
    let isolation = required_headers("data/abc/web/index.html");
    assert!(isolation.contains(&("Cross-Origin-Opener-Policy", "same-origin")));
    assert!(isolation.contains(&("Cross-Origin-Embedder-Policy", "require-corp")));
    assert_eq!(
        required_headers("data/abc/web/sw.js"),
        &[("Service-Worker-Allowed", "/")]
    );
    assert!(required_headers("data/abc/web/app.wasm").is_empty());
    assert!(required_headers("data/abc/be-x86_64-unknown-linux-gnu/be").is_empty());
    assert!(required_headers("index.html").is_empty());
    assert!(required_headers("live/be-release").is_empty());
}
