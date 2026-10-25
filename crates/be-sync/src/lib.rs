pub mod disk;
pub mod index;
mod pass;
pub mod remote;
mod report;
pub mod watch;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use be_client::{ClientError, Peer};
use be_store::ObjectStore;
use be_types_host::Modules;
use uuid::Uuid;

pub use report::Report;

use crate::index::{Index, Lock};

pub const STATE_DIRECTORY: &str = ".be";
pub const ERRORS_FILE: &str = "BE_SYNC_ERRORS.txt";
pub const IGNORE_FILE: &str = ".beignore";
pub const UNEXPORTED_FILE: &str = "unexported.txt";

const CLIENT: u64 = 0x6265_2d73_796e;
const PASSES: usize = 4;

pub struct Target {
    pub root: Option<Uuid>,
}

pub async fn sync<S: ObjectStore>(
    peer: &Peer<S>,
    types: &mut Modules,
    directory: &Path,
    target: &Target,
) -> Result<Report, String> {
    Kept::open(peer, directory, target)?.sync(peer, types).await
}

pub(crate) struct Kept {
    directory: PathBuf,
    index: Index,
    _lock: Lock,
}

impl Kept {
    pub(crate) fn open<S: ObjectStore>(
        peer: &Peer<S>,
        directory: &Path,
        target: &Target,
    ) -> Result<Self, String> {
        open(peer, directory, target)?;
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("could not make {}: {error}", directory.display()))?;
        let lock = Lock::take(directory)?;
        Ok(Self {
            directory: directory.to_owned(),
            index: open(peer, directory, target)?,
            _lock: lock,
        })
    }

    pub(crate) async fn sync<S: ObjectStore>(
        &mut self,
        peer: &Peer<S>,
        types: &mut Modules,
    ) -> Result<Report, String> {
        let mut report = Report::default();
        for _ in 0..PASSES {
            let pass = pass::run(peer, types, &self.directory, &mut self.index).await?;
            let again = pass.retry;
            report.absorb(pass);
            if !again {
                break;
            }
        }
        Ok(report)
    }
}

fn open<S: ObjectStore>(
    peer: &Peer<S>,
    directory: &Path,
    target: &Target,
) -> Result<Index, String> {
    let organization = peer.organization();
    match (Index::load(directory)?, target.root) {
        (Some(index), Some(root)) if index.root != root => Err(format!(
            "{} is kept in step with block {}, not {root}",
            directory.display(),
            index.root
        )),
        (Some(index), _) if index.organization != organization => Err(format!(
            "{} is kept in step with organization {}, not {organization}",
            directory.display(),
            index.organization
        )),
        (Some(index), _) => Ok(index),
        (None, Some(root)) => Ok(Index::new(organization, root)),
        (None, None) => Err(format!(
            "{} is not kept in step with a block yet; name one: be FOLDER sync {}",
            directory.display(),
            directory.display()
        )),
    }
}

pub(crate) fn explain(error: ClientError) -> String {
    match error {
        ClientError::Refused(_, message) => message,
        other => other.to_string(),
    }
}
