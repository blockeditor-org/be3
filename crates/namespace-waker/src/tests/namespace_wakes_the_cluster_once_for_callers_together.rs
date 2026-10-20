use std::os::unix::fs::PermissionsExt;

use super::*;

async fn get(address: SocketAddr, method: &str) -> (String, String) {
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(format!("{method} /build/namespace HTTP/1.1\r\nHost: test\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.lines().next().unwrap().to_owned(), body.to_owned())
}

#[tokio::test]
async fn namespace_wakes_the_cluster_once_for_callers_together() {
    let directory = std::env::temp_dir().join(format!("namespace-waker-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let nsc = directory.join("nsc");
    std::fs::write(
        &nsc,
        "#!/bin/sh\n\
         echo \"$NS_DO_NOT_UPDATE $*\" >> \"$(dirname \"$0\")/runs\"\n\
         while [ $# -gt 0 ]; do\n\
           if [ \"$1\" = --config ]; then\n\
             printf 'engine_address = grpcs://reapih.example:443\\ncas_address = grpcs://storageh.example:443\\nhttp_headers = x-nsc-ingress-auth:secret\\n' > \"$2\"\n\
           fi\n\
           shift\n\
         done\n",
    )
    .unwrap();
    std::fs::set_permissions(&nsc, std::fs::Permissions::from_mode(0o755)).unwrap();
    let waker = Arc::new(Waker::new(
        nsc,
        directory.join("token.json"),
        directory.join("buck2.buckconfig"),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(serve(listener, waker, async {
        let _ = stopped.await;
    }));

    let (first, second, third) = tokio::join!(
        get(address, "GET"),
        get(address, "GET"),
        get(address, "GET")
    );
    let answers = [first, second, third];
    for (status, body) in &answers {
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(body.contains("engine_address = grpcs://reapih.example:443"), "{body}");
        assert!(body.contains("http_headers = x-nsc-ingress-auth:secret"), "{body}");
    }
    assert_eq!(get(address, "GET").await.1, answers[0].1);
    assert_eq!(
        get(address, "POST").await.0,
        "HTTP/1.1 405 Method Not Allowed"
    );

    let runs = std::fs::read_to_string(directory.join("runs")).unwrap();
    let token = directory.join("token.json");
    let config = directory.join("buck2.buckconfig");
    assert_eq!(
        runs,
        format!(
            "1 reapi setup buck2 --token {} --key buildserver --config {}\n",
            token.display(),
            config.display()
        )
    );

    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
