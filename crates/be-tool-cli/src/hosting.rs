//! Runs an app's host component headless (plans/headless_apps.md): the component of the app
//! whose manifest's `hosts` names the block's type, through be-host-runtime with no window,
//! presenter or gpu (`be_host_runtime::Headless`), on a block stack the caller started. The
//! app is staged beside `be`, or downloaded from a deployed `be`'s release as the runtime reads
//! it (`crate::release`).

use std::{path::Path, sync::Arc, time::Duration};

use be_sdk_api::EditorInstanceId;
use uuid::Uuid;

/// A host component running on one block, until dropped.
pub struct Hosting {
    headless: be_host_runtime::Headless,
    app: String,
    instance: EditorInstanceId,
}

impl Hosting {
    /// Starts the host component for `block`, of `block_type`, as the device `client`, its app
    /// downloaded into `cache` if it is not there already.
    pub fn start(
        block: Uuid,
        block_type: Uuid,
        client: Uuid,
        cache: &Path,
    ) -> Result<Self, String> {
        crate::release::serve(cache)?;
        let (app, host) = be_host_stack::discovery::manifests()
            .into_iter()
            .find_map(|manifest| {
                let host = manifest
                    .hosts
                    .iter()
                    .find(|host| Uuid::from_bytes(host.edits) == block_type)?;
                Some((Arc::clone(&manifest), Uuid::from_bytes(host.id)))
            })
            .ok_or_else(|| {
                format!(
                    "no app of this build hosts blocks of type {block_type}: a be built here has \
                     only the apps staged beside it (`./scripts/buck run //crates/be-tool-cli:dev`, \
                     with `-c be3.apps=NAME,...` for more), and a deployed one its release's"
                )
            })?;
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
            return Err(format!(
                "{}'s host component could not start: {error}",
                app.identity.id
            ));
        }
        Ok(Self {
            headless,
            app: app.identity.id.clone(),
            instance,
        })
    }

    /// Runs the component until `done` answers, for at most `within`.
    pub fn run_until<T>(&mut self, within: Duration, done: impl FnMut() -> Option<T>) -> Option<T> {
        self.headless.run_until(within, done)
    }
}

impl Drop for Hosting {
    fn drop(&mut self) {
        be_host_runtime::close(&self.app, self.instance);
        self.headless.step();
    }
}
