//! Runs an app's host component headless (plans/headless_apps.md): the component of the app
//! whose manifest's `hosts` names the block's type, through be-host-runtime with no window,
//! presenter or gpu (`be_host_runtime::Headless`), on a block stack the caller started. `be`
//! carries the apps that declare `hosts` in its executable, staged as be-host stages its apps
//! and packed into one file (the `host-apps` in its BUCK), and serves them to
//! `be_host_stack::discovery` from there (`serve_apps`).

use std::{collections::HashMap, sync::Arc, time::Duration};

use be_sdk_api::EditorInstanceId;
use uuid::Uuid;

/// Serves the staged apps from `packed`, a directory packed into one file (crates/be-tool-buck's
/// `pack`), to `be_host_stack::discovery`, which then reads them as it reads a bundle beside
/// the executable. Called once, before anything discovers the apps.
pub fn serve_apps(packed: &'static [u8]) -> Result<(), String> {
    let files = unpack(packed)?;
    be_host_stack::discovery::use_assets(move |path| {
        files
            .get(path)
            .map(|contents| contents.to_vec())
            .ok_or_else(|| format!("be carries no {path}"))
    });
    Ok(())
}

/// The files of `packed`, by their path in the directory it was packed from.
pub fn unpack(packed: &'static [u8]) -> Result<HashMap<&'static str, &'static [u8]>, String> {
    let mut rest = packed;
    let mut files = HashMap::new();
    while !rest.is_empty() {
        let path = crate::apps::take(&mut rest)?;
        let path = std::str::from_utf8(path).map_err(|error| error.to_string())?;
        files.insert(path, crate::apps::take(&mut rest)?);
    }
    Ok(files)
}

/// A host component running on one block, until dropped.
pub struct Hosting {
    headless: be_host_runtime::Headless,
    app: String,
    instance: EditorInstanceId,
}

impl Hosting {
    /// Starts the host component for `block`, of `block_type`, as the device `client`.
    pub fn start(block: Uuid, block_type: Uuid, client: Uuid) -> Result<Self, String> {
        let (app, host) = be_host_stack::discovery::manifests()
            .into_iter()
            .find_map(|manifest| {
                let host = manifest
                    .hosts
                    .iter()
                    .find(|host| Uuid::from_bytes(host.edits) == block_type)?;
                Some((Arc::clone(&manifest), Uuid::from_bytes(host.id)))
            })
            .ok_or_else(|| format!("no app be carries hosts blocks of type {block_type}"))?;
        let headless = be_host_runtime::Headless::start();
        let instance = EditorInstanceId(1);
        if let be_host_runtime::CreationState::Failed(error) = be_host_runtime::host(
            &app,
            &Arc::new(be_host_runtime::AppCatalog::default()),
            client,
            instance,
            host,
            be_host_runtime::EditorBlock {
                id: block,
                block_type,
                view_block: None,
            },
        ) {
            return Err(error);
        }
        Ok(Self {
            headless,
            app: app.identity.id.clone(),
            instance,
        })
    }

    /// Runs the component until `done` answers, for at most `within`.
    pub fn run_until<T>(
        &mut self,
        within: Duration,
        done: impl FnMut() -> Option<T>,
    ) -> Option<T> {
        self.headless.run_until(within, done)
    }
}

impl Drop for Hosting {
    fn drop(&mut self) {
        be_host_runtime::close(&self.app, self.instance);
        self.headless.step();
    }
}
