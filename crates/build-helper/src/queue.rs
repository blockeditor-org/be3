use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{self, Read as _, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;
use tokio::sync::Notify;

use crate::products::{Product, State, Status, manifest_url};

pub trait Backend: Send + Sync + 'static {
    fn published(&self, product: &str, commit: &str) -> bool;

    fn allowed(&self, commit: &str) -> Result<(), String>;

    fn build(&self, product: Product, commit: &str, log: &Log) -> Result<(), String>;
}

type Key = (String, String);

struct Entry {
    product: Product,
    state: State,
    error: Option<String>,
    length: u64,
}

#[derive(Default)]
struct Inner {
    builds: HashMap<Key, Entry>,
    queue: VecDeque<Key>,
    running: bool,
}

pub struct Builds {
    backend: Arc<dyn Backend>,
    logs: PathBuf,
    public: String,
    inner: Mutex<Inner>,
    changed: Notify,
    queued: Notify,
}

pub enum Asked {
    Status(Status),
    Refused(String),
}

pub struct LogRead {
    pub state: Option<State>,
    pub length: u64,
    pub bytes: Vec<u8>,
}

#[derive(Deserialize, serde::Serialize)]
struct Finished {
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub const LONG_POLL: Duration = Duration::from_secs(25);

impl Builds {
    pub fn new(backend: Arc<dyn Backend>, state: &Path, public: String) -> Arc<Self> {
        Arc::new(Self {
            backend,
            logs: state.join("logs"),
            public,
            inner: Mutex::new(Inner::default()),
            changed: Notify::new(),
            queued: Notify::new(),
        })
    }

    fn log_path(&self, product: &str, commit: &str) -> PathBuf {
        self.logs.join(product).join(format!("{commit}.log"))
    }

    fn finished_path(&self, product: &str, commit: &str) -> PathBuf {
        self.logs.join(product).join(format!("{commit}.json"))
    }

    fn status(&self, inner: &Inner, key: &Key, entry: &Entry) -> Status {
        let position = (entry.state == State::Queued).then(|| {
            inner
                .queue
                .iter()
                .take_while(|queued| *queued != key)
                .count()
                + usize::from(inner.running)
        });
        Status {
            product: key.0.clone(),
            commit: key.1.clone(),
            state: entry.state,
            position,
            manifest: (entry.state == State::Succeeded)
                .then(|| manifest_url(&self.public, &key.0, &key.1)),
            error: entry.error.clone(),
        }
    }

    fn known(&self, product: Product, commit: &str, published: bool) -> Option<Status> {
        let key = (product.name(), commit.to_owned());
        {
            let mut inner = self.inner.lock().unwrap();
            if !inner.builds.contains_key(&key)
                && let Some(finished) = fs::read(self.finished_path(&key.0, commit))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Finished>(&bytes).ok())
            {
                let length = fs::metadata(self.log_path(&key.0, commit))
                    .map_or(0, |metadata| metadata.len());
                inner.builds.insert(
                    key.clone(),
                    Entry {
                        product,
                        state: finished.state,
                        error: finished.error,
                        length,
                    },
                );
            }
            if let Some(entry) = inner.builds.get(&key) {
                return Some(self.status(&inner, &key, entry));
            }
        }
        if !published || !self.backend.published(&key.0, commit) {
            return None;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.builds.entry(key.clone()).or_insert(Entry {
            product,
            state: State::Succeeded,
            error: None,
            length: 0,
        });
        let entry = &inner.builds[&key];
        Some(self.status(&inner, &key, entry))
    }

    pub fn get(&self, product: Product, commit: &str) -> Option<Status> {
        self.known(product, commit, true)
    }

    pub fn ask(&self, product: Product, commit: &str) -> Asked {
        if let Some(status) = self.known(product, commit, true)
            && status.state != State::Failed
        {
            return Asked::Status(status);
        }
        if let Err(error) = self.backend.allowed(commit) {
            return Asked::Refused(error);
        }
        let key = (product.name(), commit.to_owned());
        let mut inner = self.inner.lock().unwrap();
        if inner
            .builds
            .get(&key)
            .is_none_or(|entry| entry.state == State::Failed)
        {
            let _ = fs::remove_file(self.finished_path(&key.0, commit));
            let _ = fs::remove_file(self.log_path(&key.0, commit));
            inner.builds.insert(
                key.clone(),
                Entry {
                    product,
                    state: State::Queued,
                    error: None,
                    length: 0,
                },
            );
            inner.queue.push_back(key.clone());
            tracing::info!(product = key.0, commit = key.1, "queued");
            self.queued.notify_one();
            self.changed.notify_waiters();
        }
        let entry = &inner.builds[&key];
        Asked::Status(self.status(&inner, &key, entry))
    }

    pub async fn read_log(&self, product: Product, commit: &str, from: u64) -> Option<LogRead> {
        let deadline = tokio::time::Instant::now() + LONG_POLL;
        let key = (product.name(), commit.to_owned());
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let (state, length) = match self.known(product, commit, false) {
                Some(_) => {
                    let inner = self.inner.lock().unwrap();
                    let entry = &inner.builds[&key];
                    (Some(entry.state), entry.length)
                }
                None => {
                    let path = self.log_path(&key.0, commit);
                    let length = fs::metadata(&path).ok()?.len();
                    (None, length)
                }
            };
            let waiting = state.is_some_and(|state| !state.finished());
            if length > from || !waiting || tokio::time::Instant::now() >= deadline {
                let bytes = if length > from {
                    read_range(&self.log_path(&key.0, commit), from, length).unwrap_or_default()
                } else {
                    Vec::new()
                };
                return Some(LogRead {
                    state,
                    length,
                    bytes,
                });
            }
            let _ = tokio::time::timeout_at(deadline, notified).await;
        }
    }

    pub async fn run(self: Arc<Self>) {
        loop {
            let next = {
                let mut inner = self.inner.lock().unwrap();
                inner.queue.pop_front().map(|key| {
                    let entry = inner.builds.get_mut(&key).unwrap();
                    entry.state = State::Running;
                    let product = entry.product;
                    inner.running = true;
                    (key, product)
                })
            };
            let Some((key, product)) = next else {
                self.queued.notified().await;
                continue;
            };
            self.changed.notify_waiters();
            tracing::info!(product = key.0, commit = key.1, "building");
            let builds = Arc::clone(&self);
            let building = key.clone();
            let result = tokio::task::spawn_blocking(move || {
                let log = Log::open(builds, building.clone())
                    .map_err(|error| format!("could not open the build's log: {error}"))?;
                log.builds.backend.build(product, &building.1, &log)
            })
            .await
            .unwrap_or_else(|error| Err(format!("the build panicked: {error}")));
            let finished = match result {
                Ok(()) => Finished {
                    state: State::Succeeded,
                    error: None,
                },
                Err(error) => Finished {
                    state: State::Failed,
                    error: Some(
                        error
                            .lines()
                            .find(|line| !line.trim().is_empty())
                            .unwrap_or("the build failed")
                            .to_owned(),
                    ),
                },
            };
            tracing::info!(
                product = key.0,
                commit = key.1,
                state = finished.state.name(),
                error = finished.error.as_deref().unwrap_or_default(),
                "finished"
            );
            let path = self.finished_path(&key.0, &key.1);
            if let Err(error) = fs::write(&path, serde_json::to_vec(&finished).unwrap()) {
                tracing::error!("{}: {error}", path.display());
            }
            {
                let mut inner = self.inner.lock().unwrap();
                inner.running = false;
                let entry = inner.builds.get_mut(&key).unwrap();
                entry.state = finished.state;
                entry.error = finished.error;
            }
            self.changed.notify_waiters();
        }
    }
}

fn read_range(path: &Path, from: u64, to: u64) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::new();
    file.take(to - from).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub struct Log {
    builds: Arc<Builds>,
    key: Key,
    file: Mutex<fs::File>,
}

impl Log {
    fn open(builds: Arc<Builds>, key: Key) -> io::Result<Self> {
        let path = builds.log_path(&key.0, &key.1);
        fs::create_dir_all(path.parent().unwrap())?;
        let file = fs::File::create(&path)?;
        Ok(Self {
            builds,
            key,
            file: Mutex::new(file),
        })
    }

    pub fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut file = self.file.lock().unwrap();
        if let Err(error) = file.write_all(bytes) {
            tracing::error!("could not write a build's log: {error}");
            return;
        }
        let mut inner = self.builds.inner.lock().unwrap();
        if let Some(entry) = inner.builds.get_mut(&self.key) {
            entry.length += bytes.len() as u64;
        }
        drop(inner);
        drop(file);
        self.builds.changed.notify_waiters();
    }

    pub fn line(&self, text: &str) {
        self.write(format!("{text}\n").as_bytes());
    }
}
