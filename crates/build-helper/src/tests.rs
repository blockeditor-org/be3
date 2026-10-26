use std::{
    collections::HashSet,
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, mpsc},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc::UnboundedReceiver, mpsc::UnboundedSender, oneshot},
    time::Duration,
};

use super::*;
use crate::{
    namespace::Probe,
    queue::{Backend, Log},
};

mod a_failed_build_says_why_and_is_built_again_when_asked;
mod a_kept_answer_is_handed_out_until_its_executor_stops_answering;
mod a_published_manifest_answers_succeeded_without_queueing;
mod asking_twice_queues_one_build;
mod commits_not_on_a_branch_are_refused;
mod namespace_is_served_only_with_a_token;
mod namespace_wakes_the_cluster_once_for_callers_together;
mod the_log_waits_for_new_bytes_and_says_the_final_state;
mod the_position_counts_the_builds_ahead;
mod unknown_products_and_commits_are_not_found;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "89abcdef0123456789abcdef0123456789abcdef";
const THIRD: &str = "fedcba9876543210fedcba9876543210fedcba98";

struct Fake {
    published: Mutex<HashSet<String>>,
    refused: HashSet<String>,
    built: Mutex<Vec<String>>,
    started: UnboundedSender<String>,
    finish: Mutex<mpsc::Receiver<Result<(), String>>>,
}

impl Backend for Fake {
    fn published(&self, product: &str, commit: &str) -> bool {
        self.published
            .lock()
            .unwrap()
            .contains(&format!("{product}/{commit}"))
    }

    fn allowed(&self, commit: &str) -> Result<(), String> {
        if self.refused.contains(commit) {
            Err(format!("{commit} is not on a branch"))
        } else {
            Ok(())
        }
    }

    fn build(&self, product: products::Product, commit: &str, log: &Log) -> Result<(), String> {
        let name = format!("{}/{commit}", product.name());
        self.built.lock().unwrap().push(name.clone());
        log.line(&format!("building {name}"));
        let _ = self.started.send(name.clone());
        let result = self
            .finish
            .lock()
            .unwrap()
            .recv()
            .unwrap_or_else(|_| Err("stopped".into()));
        log.line("done");
        if result.is_ok() {
            self.published.lock().unwrap().insert(name);
        }
        result
    }
}

struct Response {
    status: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

struct Running {
    directory: PathBuf,
    address: SocketAddr,
    waker: Arc<Waker>,
    fake: Arc<Fake>,
    started: UnboundedReceiver<String>,
    finish: mpsc::Sender<Result<(), String>>,
    stop: oneshot::Sender<()>,
    server: tokio::task::JoinHandle<io::Result<()>>,
}

async fn start(name: &str, probe: Option<Probe>) -> Running {
    start_with(name, probe, true, &[]).await
}

async fn start_with(name: &str, probe: Option<Probe>, token: bool, refused: &[&str]) -> Running {
    let directory =
        std::env::temp_dir().join(format!("build-helper-test-{name}-{}", std::process::id()));
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
    let (started_sender, started) = tokio::sync::mpsc::unbounded_channel();
    let (finish, finishing) = mpsc::channel();
    let fake = Arc::new(Fake {
        published: Mutex::default(),
        refused: refused.iter().map(|commit| (*commit).to_owned()).collect(),
        built: Mutex::default(),
        started: started_sender,
        finish: Mutex::new(finishing),
    });
    let builds = queue::Builds::new(
        Arc::clone(&fake) as Arc<dyn Backend>,
        &directory.join("state"),
        "https://cdn.example".into(),
    );
    let service = Arc::new(Service {
        waker: token.then(|| Arc::clone(&waker)),
        builds: Some(builds),
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(serve(listener, service, async {
        let _ = stopped.await;
    }));
    Running {
        directory,
        address,
        waker,
        fake,
        started,
        finish,
        stop,
        server,
    }
}

impl Running {
    async fn request(&self, method: &str, path: &str) -> Response {
        let mut stream = TcpStream::connect(self.address).await.unwrap();
        stream
            .write_all(format!("{method} {path} HTTP/1.1\r\nHost: test\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let mut lines = head.lines();
        let status = lines.next().unwrap().to_owned();
        let headers = lines
            .filter_map(|line| line.split_once(": "))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        Response {
            status,
            headers,
            body: body.to_owned(),
        }
    }

    async fn get(&self, method: &str) -> (String, Option<u64>, String) {
        let response = self.request(method, "/build/namespace").await;
        let age = response.header("age").map(|age| age.parse().unwrap());
        (response.status, age, response.body)
    }

    async fn product(&self, method: &str, product: &str, commit: &str) -> Response {
        self.request(method, &format!("/build/products/{product}/{commit}"))
            .await
    }

    async fn started(&mut self) -> String {
        tokio::time::timeout(Duration::from_secs(30), self.started.recv())
            .await
            .unwrap()
            .unwrap()
    }

    fn finish(&self, result: Result<(), String>) {
        self.finish.send(result).unwrap();
    }

    async fn finished(&self, product: &str, commit: &str) -> Response {
        let mut from = 0;
        loop {
            let log = self
                .request(
                    "GET",
                    &format!("/build/products/{product}/{commit}/log?from={from}"),
                )
                .await;
            if matches!(log.header("build-state"), Some("succeeded" | "failed")) {
                return log;
            }
            from = log.header("build-log-length").unwrap().parse().unwrap();
        }
    }

    fn built(&self) -> Vec<String> {
        self.fake.built.lock().unwrap().clone()
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
