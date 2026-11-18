//! Runs an app's host component headless (plans/headless_apps.md): the component of the app
//! whose manifest's `hosts` names the block's type, through be-host-runtime with no window,
//! presenter or gpu (`be_host_runtime::Headless`), on a block stack the caller started. The
//! app is staged beside `be`, or downloaded from a deployed `be`'s release as the runtime reads
//! it (`crate::release`).

use std::{sync::Arc, time::Duration};

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
    /// read as `crate::release::serve` set discovery up to.
    pub fn start(block: Uuid, block_type: Uuid, client: Uuid) -> Result<Self, String> {
        let (app, host) = host_of(block_type)?;
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

/// Hosts `block`, of `block_type`, as the device `client` (`be BLOCK host`, plans/headless_apps.md,
/// step 2), on a block stack the caller started, until `stop` answers true: holds the block's
/// replica open, so this device is in its session, starts the type's host component whenever
/// no one hosts it (starting claims the lease, plans/hosted_blocks.md, step 4d2), and stops it,
/// standing by, while another device does. `told` hears each change.
pub fn host_block(
    block: Uuid,
    block_type: Uuid,
    client: Uuid,
    mut stop: impl FnMut() -> bool,
    mut told: impl FnMut(&str),
) -> Result<(), String> {
    let (app, host) = host_of(block_type)?;
    let mut headless = be_host_runtime::Headless::start();
    be_host_stack::open(block, block_type);
    let editor_block = be_host_runtime::EditorBlock {
        id: block,
        block_type,
        view_block: None,
    };
    let catalog = Arc::new(be_host_runtime::AppCatalog::default());
    let mut instances = 0;
    // The component running, and whether its app has opened it yet.
    let mut running: Option<(EditorInstanceId, bool)> = None;
    told("standing by until no one else hosts it");
    while !stop() {
        be_host_stack::take_updates_in_frame();
        let may_host = be_host_stack::may_host(block);
        match running {
            None if may_host => {
                instances += 1;
                be_host_stack::restart_host(block);
                running = Some((EditorInstanceId(instances), false));
                told("hosting");
            }
            Some((instance, _)) if !may_host => {
                be_host_runtime::close(&app.identity.id, instance);
                running = None;
                told("another device hosts it; standing by");
            }
            _ => {}
        }
        // Reported, as be-host reports a component, until its app opens it.
        if let Some((instance, false)) = running {
            match be_host_runtime::host(&app, &catalog, client, instance, host, editor_block) {
                be_host_runtime::CreationState::Ready => running = Some((instance, true)),
                be_host_runtime::CreationState::Failed(error) => {
                    be_host_stack::close(block);
                    return Err(format!(
                        "{}'s host component could not start: {error}",
                        app.identity.id
                    ));
                }
                _ => {}
            }
        }
        headless.run_until(Duration::from_millis(250), || None::<()>);
    }
    if let Some((instance, _)) = running {
        be_host_runtime::close(&app.identity.id, instance);
        headless.step();
    }
    be_host_stack::close(block);
    Ok(())
}

/// The app whose manifest's `hosts` names `block_type`, and its host component.
fn host_of(block_type: Uuid) -> Result<(Arc<be_sdk_manifest::AppManifest>, Uuid), String> {
    crate::apps::found()
        .manifests()
        .iter()
        .find_map(|manifest| {
            let host = manifest
                .hosts
                .iter()
                .find(|host| Uuid::from_bytes(host.edits) == block_type)?;
            Some((Arc::clone(manifest), Uuid::from_bytes(host.id)))
        })
        .ok_or_else(|| {
            format!(
                "no app of this build hosts blocks of type {block_type}: a be built here has \
                 only the apps staged beside it (`./scripts/buck run //crates/be-tool-cli:dev`, \
                 with `-c be3.apps=NAME,...` for more), and a deployed one its release's"
            )
        })
}

impl Drop for Hosting {
    fn drop(&mut self) {
        be_host_runtime::close(&self.app, self.instance);
        self.headless.step();
    }
}
