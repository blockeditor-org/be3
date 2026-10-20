use std::{net::SocketAddr, os::unix::fs::PermissionsExt};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
};

use super::*;

mod a_kept_answer_is_handed_out_until_its_executor_stops_answering;
mod namespace_wakes_the_cluster_once_for_callers_together;

struct Running {
    directory: PathBuf,
    address: SocketAddr,
    waker: Arc<Waker>,
    stop: oneshot::Sender<()>,
    server: tokio::task::JoinHandle<io::Result<()>>,
}

async fn start(name: &str, probe: Option<Probe>) -> Running {
    let directory = std::env::temp_dir().join(format!(
        "namespace-waker-test-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let nsc = directory.join("nsc");
    fs::write(
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
    fs::set_permissions(&nsc, fs::Permissions::from_mode(0o755)).unwrap();
    let mut waker = Waker::new(nsc, directory.join("token.json"), directory.join("work"));
    if let Some(probe) = probe {
        waker.probe = probe;
    }
    let waker = Arc::new(waker);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(serve(listener, Arc::clone(&waker), async {
        let _ = stopped.await;
    }));
    Running {
        directory,
        address,
        waker,
        stop,
        server,
    }
}

impl Running {
    async fn get(&self, method: &str) -> (String, Option<u64>, String) {
        let mut stream = TcpStream::connect(self.address).await.unwrap();
        stream
            .write_all(
                format!("{method} /build/namespace HTTP/1.1\r\nHost: test\r\n\r\n").as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let age = head
            .lines()
            .find_map(|line| line.strip_prefix("age: "))
            .map(|age| age.parse().unwrap());
        (
            head.lines().next().unwrap().to_owned(),
            age,
            body.to_owned(),
        )
    }

    fn runs(&self) -> usize {
        fs::read_to_string(self.directory.join("runs"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    async fn age_by(&self, by: Duration) {
        let mut answer = self.waker.answer.lock().await;
        let (at, _) = answer.as_mut().unwrap();
        *at = at.checked_sub(by).unwrap();
    }

    async fn stop(self) {
        self.stop.send(()).unwrap();
        self.server.await.unwrap().unwrap();
        fs::remove_dir_all(self.directory).unwrap();
    }
}
