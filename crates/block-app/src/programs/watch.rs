use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use uuid::Uuid;

pub(crate) struct Watch {
    watcher: Option<RecommendedWatcher>,
    touched: Receiver<PathBuf>,
    watched: HashMap<PathBuf, PathBuf>,
}

impl Watch {
    pub(crate) fn new() -> Self {
        let (sender, touched) = block_app_base::host::waking_channel();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event
                && (event.kind.is_modify() || event.kind.is_create())
            {
                for path in event.paths {
                    let _ = sender.send(path);
                }
            }
        });
        let watcher = match watcher {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                eprintln!("block-app: files open in programs are not watched: {error}");
                None
            }
        };
        Self {
            watcher,
            touched,
            watched: HashMap::new(),
        }
    }

    pub(crate) fn add(&mut self, file: &Path) {
        let Some(dir) = file.parent() else {
            return;
        };
        if self
            .watched
            .insert(file.to_owned(), dir.to_owned())
            .is_some()
        {
            return;
        }
        if let Some(watcher) = &mut self.watcher
            && let Err(error) = watcher.watch(dir, RecursiveMode::NonRecursive)
        {
            eprintln!("block-app: {} is not watched: {error}", dir.display());
        }
    }

    pub(crate) fn forget(&mut self, block: Uuid) {
        let block = block.to_string();
        let gone: Vec<PathBuf> = self
            .watched
            .iter()
            .filter(|(_, dir)| dir.file_name().is_some_and(|name| name == block.as_str()))
            .map(|(file, _)| file.clone())
            .collect();
        for file in gone {
            if let Some(dir) = self.watched.remove(&file)
                && let Some(watcher) = &mut self.watcher
            {
                let _ = watcher.unwatch(&dir);
            }
        }
    }

    pub(crate) fn take(&mut self) -> HashSet<PathBuf> {
        let mut touched = HashSet::new();
        while let Ok(path) = self.touched.try_recv() {
            if self.watched.contains_key(&path) {
                touched.insert(path);
            }
        }
        touched
    }
}
