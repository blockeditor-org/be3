use super::*;

#[tokio::test]
async fn the_log_waits_for_new_bytes_and_says_the_final_state() {
    let mut running = start("log", None).await;
    let path = format!("/build/products/launcher-android/{COMMIT}/log");

    running.product("POST", "launcher-android", COMMIT).await;
    running.started().await;
    let building = format!("building launcher-android/{COMMIT}\n");
    let first = running.request("GET", &format!("{path}?from=0")).await;
    assert_eq!(first.status, "HTTP/1.1 200 OK");
    assert_eq!(first.body, building);
    assert_eq!(first.header("build-state"), Some("running"));
    let length = building.len();
    assert_eq!(
        first.header("build-log-length"),
        Some(length.to_string().as_str())
    );

    let after = format!("{path}?from={length}");
    let waiting = running.request("GET", &after);
    let finishing = async {
        running.finish(Ok(()));
    };
    let (next, ()) = tokio::join!(waiting, finishing);
    assert_eq!(next.body, "done\n");

    let total = length + "done\n".len();
    let last = running
        .request("GET", &format!("{path}?from={total}"))
        .await;
    assert_eq!(last.header("build-state"), Some("succeeded"));
    assert_eq!(last.body, "");
    assert_eq!(
        last.header("build-log-length"),
        Some(total.to_string().as_str())
    );

    let whole = running.request("GET", &path).await;
    assert_eq!(whole.body, format!("{building}done\n"));

    running.stop().await;
}
