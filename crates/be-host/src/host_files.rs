//! The host's own files, before and beside any block stack: its state (`host.sqlite3`) and the
//! organizations it last listed for each saved account (the refs database's cache), which the
//! Accounts and Organizations screens show while no stack runs.
//!
//! The host reads both from copies it holds (`HostStateStore`, `WorkspaceCache`), and each
//! change goes to the copy at once and to where it is kept. A native host keeps them itself,
//! in this process. On the web the page makes no file call (plans/web_page_never_waits.md):
//! the host's worker (`web/be-worker.js`, which runs the block stacks too) opens them, hands
//! the page what they hold once (`open`), and keeps each change the page sends after
//! (decisions/web_host_state_in_a_worker.md).

use std::collections::HashMap;

use be_block_protocol::Organization;
use be_host_base::host_state::{SavedAccount, ServerLocation};
use uuid::Uuid;

/// The organizations each saved account last listed, by server address and account.
type Listed = HashMap<(String, Uuid), Vec<Organization>>;

/// Who keeps the organizations listed: the refs database here, or the web's host worker.
enum Keep {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Here(Option<be_block_client::RefsDb>),
    #[cfg(target_arch = "wasm32")]
    Elsewhere(tokio::sync::mpsc::UnboundedSender<web::Job>),
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

/// The server address an account's organizations are cached under: a local account's is the
/// embedded server's.
fn address(account: &SavedAccount, local: &str) -> String {
    match &account.server {
        ServerLocation::Local => local.to_owned(),
        ServerLocation::Remote(url) => url.clone(),
    }
}

/// What `refs` lists for each of `accounts`.
fn read(refs: &be_block_client::RefsDb, accounts: &[SavedAccount], local: &str) -> Listed {
    accounts
        .iter()
        .map(|account| {
            let url = address(account, local);
            let listed = refs.workspaces(&url, account.id).unwrap_or_default();
            ((url, account.id), listed)
        })
        .collect()
}

impl WorkspaceCache {
    /// Reads the refs database in `data_dir` here, in this process.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open(data_dir: &std::path::Path, accounts: &[SavedAccount], local: &str) -> Self {
        let refs = be_host_stack::refs_in(data_dir).ok();
        Self {
            listed: refs
                .as_ref()
                .map(|refs| read(refs, accounts, local))
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
pub(crate) use web::open;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::path::PathBuf;

    use be_block_protocol::Organization;
    use be_host_base::host_state::{Held, HostStateStore, Keeper, Write};
    use tokio::sync::{mpsc, oneshot};
    use uuid::Uuid;

    use super::{Keep, Listed, WorkspaceCache, read};

    /// A change the page sends the host's worker to keep.
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

    /// Starts the host's worker on the host's files in `data_dir`, and waits, without blocking
    /// the page, for what they hold.
    pub(crate) async fn open(data_dir: PathBuf) -> Result<(HostStateStore, WorkspaceCache), String> {
        let (answer, answered) = oneshot::channel::<Opened>();
        let (jobs, taken) = mpsc::unbounded_channel();
        be_host_stack::run_in_worker(move || Box::pin(keep(data_dir, answer, taken)));
        let (held, listed) = answered
            .await
            .map_err(|_| "the host's worker stopped before it read the host's files".to_owned())??;
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
    pub(super) fn store(jobs: mpsc::UnboundedSender<Job>, held: Held) -> HostStateStore {
        HostStateStore::kept_elsewhere(held, move |write| {
            let _ = jobs.send(Job::State(write));
        })
    }

    /// In the host's worker: opens the files, answers what they hold, then keeps each change.
    async fn keep(
        data_dir: PathBuf,
        answer: oneshot::Sender<Opened>,
        mut taken: mpsc::UnboundedReceiver<Job>,
    ) {
        let opened = (|| {
            std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
            let keeper =
                Keeper::open(data_dir.join("host.sqlite3")).map_err(|error| error.to_string())?;
            let held = keeper.read().map_err(|error| error.to_string())?;
            let refs = be_host_stack::refs_in(&data_dir).ok();
            // The web runs no embedded server, so no account is a local one.
            let listed = refs
                .as_ref()
                .map(|refs| read(refs, held.accounts(), ""))
                .unwrap_or_default();
            Ok::<_, String>((keeper, refs, held, listed))
        })();
        let path = data_dir.join("host.sqlite3");
        let (mut keeper, refs) = match opened {
            Ok((keeper, refs, held, listed)) => {
                let _ = answer.send(Ok((held, listed)));
                (Some(keeper), refs)
            }
            Err(error) => {
                let _ = answer.send(Err(error));
                return;
            }
        };
        while let Some(job) = taken.recv().await {
            let kept = match job {
                Job::State(write) => match &keeper {
                    Some(keeper) => keeper.keep(&write).map_err(|error| error.to_string()),
                    None => Err("the host's state could not be opened again".to_owned()),
                },
                Job::ForgetState => {
                    // Closed before its files go.
                    keeper = None;
                    forget(&path).and_then(|()| {
                        keeper = Some(Keeper::open(&path).map_err(|error| error.to_string())?);
                        Ok(())
                    })
                }
                Job::Workspaces {
                    url,
                    account,
                    organizations,
                } => match &refs {
                    Some(refs) => refs
                        .set_workspaces(&url, account, &organizations)
                        .map_err(|error| error.to_string()),
                    None => Ok(()),
                },
            };
            if let Err(error) = kept {
                eprintln!("be-host: a change to the host's files was not kept: {error}");
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
