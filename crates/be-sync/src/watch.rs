use std::future::Future;
use std::path::{Component, Path};
use std::time::Duration;

use be_client::Peer;
use be_store::ObjectStore;
use be_types_host::Modules;
use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::{ERRORS_FILE, Kept, Report, STATE_DIRECTORY, Target};

const QUIET: Duration = Duration::from_millis(200);
const NEVER_WATCHED: [&str; 4] = [STATE_DIRECTORY, ".git", "target", "buck-out"];

pub async fn watch<S: ObjectStore>(
    peer: &Peer<S>,
    types: &mut Modules,
    directory: &Path,
    target: &Target,
    interval: Duration,
    stop: impl Future<Output = ()>,
    mut passed: impl FnMut(Result<&Report, &str>),
) -> Result<(), String> {
    let mut kept = Kept::open(peer, directory, target)?;
    let (changed, mut changes) = mpsc::unbounded_channel();
    let root = directory
        .canonicalize()
        .map_err(|error| format!("could not find {}: {error}", directory.display()))?;
    let watched_root = root.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            let _ = changed.send(());
            return;
        };
        if event
            .paths
            .iter()
            .any(|path| worth_a_pass(&watched_root, path))
        {
            let _ = changed.send(());
        }
    })
    .map_err(|error| format!("could not watch {}: {error}", directory.display()))?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| format!("could not watch {}: {error}", directory.display()))?;
    tokio::pin!(stop);
    loop {
        match kept.sync(peer, types).await {
            Ok(report) => passed(Ok(&report)),
            Err(error) => passed(Err(&error)),
        }
        while changes.try_recv().is_ok() {}
        tokio::select! {
            () = &mut stop => return Ok(()),
            () = tokio::time::sleep(interval) => {}
            Some(()) = changes.recv() => {
                loop {
                    tokio::select! {
                        () = tokio::time::sleep(QUIET) => break,
                        Some(()) = changes.recv() => {}
                    }
                }
            }
        }
    }
}

fn worth_a_pass(root: &Path, path: &Path) -> bool {
    let Ok(inside) = path.strip_prefix(root) else {
        return true;
    };
    if inside == Path::new(ERRORS_FILE) {
        return false;
    }
    !inside.components().any(|component| match component {
        Component::Normal(name) => NEVER_WATCHED.iter().any(|never| name == *never),
        _ => false,
    })
}
