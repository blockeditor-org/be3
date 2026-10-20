use super::*;

#[tokio::test]
async fn namespace_wakes_the_cluster_once_for_callers_together() {
    let running = start("together", None).await;

    let (first, second, third) =
        tokio::join!(running.get("GET"), running.get("GET"), running.get("GET"));
    let answers = [first, second, third];
    for (status, age, body) in &answers {
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(age.is_some_and(|age| age < 60), "{age:?}");
        assert!(
            body.contains("engine_address = grpcs://reapih.example:443"),
            "{body}"
        );
        assert!(
            body.contains("http_headers = x-nsc-ingress-auth:secret"),
            "{body}"
        );
    }
    assert_eq!(running.get("GET").await.2, answers[0].2);
    assert_eq!(
        running.get("POST").await.0,
        "HTTP/1.1 405 Method Not Allowed"
    );

    let runs = fs::read_to_string(running.directory.join("runs")).unwrap();
    assert_eq!(
        runs,
        format!(
            "1 reapi setup buck2 --token {} --key buildserver --config {}\n",
            running.directory.join("token.json").display(),
            running.directory.join("work/buck2.buckconfig").display()
        )
    );
    running.stop().await;
}
