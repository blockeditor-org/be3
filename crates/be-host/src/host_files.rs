//! The host's own files, before and beside any block stack: its state (`host.sqlite3`) and the
//! organizations it last listed for each saved account (the refs database's cache), which the
//! Accounts and Organizations screens show while no stack runs.
//!
//! The host reads both from copies it holds (`HostStateStore`, `WorkspaceCache`), and each
//! change goes to the copy at once and to where it is kept. A native host keeps them itself,
//! in this process. On the web the page makes no file call (decisions/the_web_page_never_waits.md):
//! the host's worker (`web/be-worker.js`, which runs the block stacks too) opens them, hands
//! the page what they hold once (`open`), and keeps each change the page sends after
//! (decisions/web_host_state_in_a_worker.md).

use std::collections::HashMap;

use be_block_protocol::Organization;
use be_host_base::host_state::SavedAccount;
use uuid::Uuid;

/// The organizations each saved account last listed, by server address and account.
type Listed = HashMap<(String, Uuid), Vec<Organization>>;

/// Who keeps the organizations listed: the refs database here, or the web's host worker.
enum Keep {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Here(Option<be_block_client::RefsDb>),
    #[cfg(target_arch = "wasm32")]
    Elsewhere(web::Jobs),
}

/// The organizations each saved account last listed, for the screens shown while no stack runs
/// and for opening one offline.
pub(crate) struct WorkspaceCache {
    listed: Listed,
    keep: Keep,
}

impl Default for WorkspaceCache {
    fn default() -> Self {
        Self {
            listed: Listed::new(),
            keep: Keep::Here(None),
        }
    }
}

/// What `refs` lists for each of `accounts`.
fn read(refs: &be_block_client::RefsDb, accounts: &[SavedAccount]) -> Listed {
    accounts
        .iter()
        .map(|account| {
            let url = account.server.url();
            let listed = refs.workspaces(&url, account.id).unwrap_or_default();
            ((url, account.id), listed)
        })
        .collect()
}

impl WorkspaceCache {
    /// Reads the refs database in `data_dir` here, in this process.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open(data_dir: &std::path::Path, accounts: &[SavedAccount]) -> Self {
        let refs = be_host_stack::refs_in(data_dir).ok();
        Self {
            listed: refs
                .as_ref()
                .map(|refs| read(refs, accounts))
                .unwrap_or_default(),
            keep: Keep::Here(refs),
        }
    }

    pub(crate) fn workspaces(&self, url: &str, account: Uuid) -> Vec<Organization> {
        self.listed
            .get(&(url.to_owned(), account))
            .cloned()
            .unwrap_or_default()
    }

    /// On the web, has the host's worker delete `host.sqlite3` and keep a fresh one, and
    /// answers the empty store whose changes go to it.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn forget_host_state(&self) -> Option<be_host_base::host_state::HostStateStore> {
        let Keep::Elsewhere(jobs) = &self.keep else {
            return None;
        };
        jobs.send(web::Job::ForgetState).ok()?;
        Some(web::store(jobs.clone(), Default::default()))
    }

    pub(crate) fn set(&mut self, url: &str, account: Uuid, organizations: Vec<Organization>) {
        match &self.keep {
            Keep::Here(refs) => {
                if let Some(refs) = refs {
                    let _ = refs.set_workspaces(url, account, &organizations);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Keep::Elsewhere(jobs) => {
                let _ = jobs.send(web::Job::Workspaces {
                    url: url.to_owned(),
                    account,
                    organizations: organizations.clone(),
                });
            }
        }
        self.listed.insert((url.to_owned(), account), organizations);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use web::{ENTRY, Unkept, keep, open};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    use be_block_protocol::Organization;
    use be_host_base::host_state::{Held, HostStateStore, Keeper, Write};
    use tokio::sync::{mpsc, oneshot};
    use uuid::Uuid;

    use super::{Keep, Listed, WorkspaceCache, read};

    /// A change the host's worker could not keep: what it was, in words for the person
    /// (`Write::what`), and why.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub(crate) struct Unkept {
        pub(crate) what: String,
        pub(crate) error: String,
    }

    /// A change the page sends the host's worker to keep, as its bytes (`Jobs`).
    #[derive(serde::Deserialize, serde::Serialize)]
    pub(crate) enum Job {
        State(Write),
        /// Deletes `host.sqlite3` and keeps the changes after in a fresh one.
        ForgetState,
        Workspaces {
            url: String,
            account: Uuid,
            organizations: Vec<Organization>,
        },
    }

    type Opened = Result<(Held, Listed), String>;

    impl Job {
        fn what(&self) -> String {
            match self {
                Job::State(write) => write.what(),
                Job::ForgetState => "deleting this device's saved data".to_owned(),
                Job::Workspaces { .. } => "the organizations you last listed".to_owned(),
            }
        }
    }

    /// Where the page sends the host's worker its jobs, each as its bytes, as a page whose
    /// memory the worker does not share sends them.
    #[derive(Clone)]
    pub(crate) struct Jobs(mpsc::UnboundedSender<Vec<u8>>);

    impl Jobs {
        /// Fails once the worker stopped taking them.
        pub(crate) fn send(&self, job: Job) -> Result<(), ()> {
            self.0
                .send(postcard::to_stdvec(&job).expect("a host job encodes"))
                .map_err(|_| ())
        }
    }

    /// Starts the host's worker on the host's files in `data_dir`, and waits, without blocking
    /// the page, for what they hold. A change it cannot keep is told to `report`, from the first
    /// one on.
    pub(crate) async fn open(
        data_dir: PathBuf,
        report: be_host_base::host::Post<Unkept>,
    ) -> Result<(HostStateStore, WorkspaceCache), String> {
        // The worker answers what the files hold on the port, its first message, and takes each
        // job after from it. A worker that drops the port unanswered drops `answer`.
        let (answer, answered) = oneshot::channel::<Vec<u8>>();
        let answer = Rc::new(RefCell::new(Some(answer)));
        let start = Start {
            data_dir,
            report: report.across(),
        };
        let mut port = be_host_stack::start_in_worker(
            ENTRY,
            postcard::to_stdvec(&start).expect("what the host's worker starts from encodes"),
        );
        port.listen(
            {
                let answer = Rc::clone(&answer);
                move |bytes| {
                    if let Some(answer) = answer.borrow_mut().take() {
                        let _ = answer.send(bytes);
                    }
                }
            },
            // The worker went: before it answered, `open` fails; after, each change from then on
            // is lost, which the person hears of once, as of a change the worker could not keep.
            move || {
                if answer.borrow_mut().take().is_none() {
                    report.send(Unkept {
                        what: "anything you change from now on".to_owned(),
                        error: "be-host's worker stopped".to_owned(),
                    });
                }
            },
        );
        let (jobs, mut taken) = mpsc::unbounded_channel::<Vec<u8>>();
        let jobs = Jobs(jobs);
        // Each job goes to the worker as it is sent, for as long as the page sends them.
        wasm_bindgen_futures::spawn_local(async move {
            while let Some(job) = taken.recv().await {
                port.post(&job);
            }
        });
        let answered = answered
            .await
            .map_err(|_| "the host's worker stopped before it read the host's files".to_owned())?;
        let (held, listed) = postcard::from_bytes::<Opened>(&answered)
            .expect("what the host's worker answers decodes")?;
        let host_state = store(jobs.clone(), held);
        Ok((
            host_state,
            WorkspaceCache {
                listed,
                keep: Keep::Elsewhere(jobs),
            },
        ))
    }

    /// The page's store of `held`, whose changes go to the host's worker.
    pub(super) fn store(jobs: Jobs, held: Held) -> HostStateStore {
        HostStateStore::kept_elsewhere(held, move |write| {
            let _ = jobs.send(Job::State(write));
        })
    }

    /// The name the host's worker keeps the host's files under (`keep`).
    pub(crate) const ENTRY: &str = "host-files";

    /// What the host's worker starts keeping the host's files from, as its bytes.
    #[derive(serde::Deserialize, serde::Serialize)]
    struct Start {
        data_dir: PathBuf,
        report: be_host_base::host::Across<Unkept>,
    }

    /// What the host's worker keeps the host's files with.
    struct Kept {
        path: PathBuf,
        keeper: Option<Keeper>,
        refs: Option<be_block_client::RefsDb>,
        taken: mpsc::UnboundedReceiver<Vec<u8>>,
        report: be_host_base::host::Across<Unkept>,
    }

    impl Kept {
        fn run(&mut self, job: Job) {
            let what = job.what();
            let kept = match job {
                Job::State(write) => match &self.keeper {
                    Some(keeper) => keeper.keep(&write).map_err(|error| error.to_string()),
                    None => Err("the host's state could not be opened again".to_owned()),
                },
                Job::ForgetState => {
                    // Closed before its files go.
                    self.keeper = None;
                    forget(&self.path).and_then(|()| {
                        self.keeper =
                            Some(Keeper::open(&self.path).map_err(|error| error.to_string())?);
                        Ok(())
                    })
                }
                Job::Workspaces {
                    url,
                    account,
                    organizations,
                } => match &self.refs {
                    Some(refs) => refs
                        .set_workspaces(&url, account, &organizations)
                        .map_err(|error| error.to_string()),
                    None => Ok(()),
                },
            };
            if let Err(error) = kept {
                self.report.send(Unkept { what, error });
            }
        }

        /// Keeps every change sent so far.
        fn run_sent(&mut self) {
            while let Ok(job) = self.taken.try_recv() {
                self.run(decode(&job));
            }
        }
    }

    /// A job the page encoded from a `Job`: bytes that do not decode are a bug.
    fn decode(job: &[u8]) -> Job {
        postcard::from_bytes(job).expect("a host job decodes")
    }

    /// In the host's worker, started as `ENTRY` from the bytes `open` sent: opens the files,
    /// answers what they hold on `port`, then keeps each change the page posts when the worker
    /// next yields or starts a stack's pass (`be_host_stack::run_first`), whichever comes first:
    /// a change waits at most for the synchronous stretch it arrived in.
    pub(crate) async fn keep(mut port: be_host_stack::Port, bytes: Vec<u8>) {
        // The page encoded them from a `Start`: bytes that do not decode are a bug.
        let Start { data_dir, report } =
            postcard::from_bytes(&bytes).expect("what the host's worker starts from decodes");
        let (sender, taken) = mpsc::unbounded_channel();
        // Dropped as the page's port goes, which ends the keeping.
        let sender = Rc::new(RefCell::new(Some(sender)));
        port.listen(
            {
                let sender = Rc::clone(&sender);
                move |job| {
                    if let Some(sender) = sender.borrow().as_ref() {
                        let _ = sender.send(job);
                    }
                }
            },
            move || drop(sender.borrow_mut().take()),
        );
        let opened = (|| {
            std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
            let keeper =
                Keeper::open(data_dir.join("host.sqlite3")).map_err(|error| error.to_string())?;
            let held = keeper.read().map_err(|error| error.to_string())?;
            let refs = be_host_stack::refs_in(&data_dir).ok();
            let listed = refs
                .as_ref()
                .map(|refs| read(refs, held.accounts()))
                .unwrap_or_default();
            Ok::<_, String>((keeper, refs, held, listed))
        })();
        let encoded =
            |opened: Opened| postcard::to_stdvec(&opened).expect("the host's files encode");
        let kept = match opened {
            Ok((keeper, refs, held, listed)) => {
                port.post(&encoded(Ok((held, listed))));
                Kept {
                    path: data_dir.join("host.sqlite3"),
                    keeper: Some(keeper),
                    refs,
                    taken,
                    report,
                }
            }
            Err(error) => {
                port.post(&encoded(Err(error)));
                return;
            }
        };
        let kept = Rc::new(RefCell::new(kept));
        be_host_stack::run_first({
            let kept = Rc::clone(&kept);
            move || kept.borrow_mut().run_sent()
        });
        loop {
            let job = std::future::poll_fn(|context| kept.borrow_mut().taken.poll_recv(context));
            match job.await {
                Some(job) => kept.borrow_mut().run(decode(&job)),
                None => return,
            }
        }
    }

    fn forget(path: &std::path::Path) -> Result<(), String> {
        for stale in [
            path.to_owned(),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            match std::fs::remove_file(&stale) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(format!("failed to delete {}: {error}", stale.display()));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
