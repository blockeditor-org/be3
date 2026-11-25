//! Runs an app's host component headless (guides/apps/runtime.md, "Headless"): the component of the
//! app whose manifest's `hosts` names the block's type, through be-host-runtime with no window,
//! presenter or gpu (`be_host_runtime::Headless`), on a block stack the caller started. The app is
//! staged beside `be`, or downloaded from a deployed `be`'s release as the runtime reads it
//! (`crate::release`).

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

/// How long `host_block` sleeps at most before it looks again: it is woken by whatever it
/// waits on (the stack handing over a change, the app making progress, `wake` from a stop),
/// so this only bounds a wait nothing woke.
const LONGEST_SLEEP: Duration = Duration::from_secs(3600);

/// Wakes a `host_block` that sleeps, from any thread: one asked to stop looks at `stop` then.
pub fn wake() {
    be_host_base::host::wake();
}

/// Hosts `block`, of `block_type`, as the device `client` (`be BLOCK host`, guides/apps/runtime.md,
/// "Headless"), on a block stack the caller started, until `stop` answers true: holds the block's
/// replica open, so this device is in its session, starts the type's host component whenever
/// no one hosts it (starting claims the lease, plans/hosted_blocks.md, step 4d2), and stops it,
/// standing by, while another device does. It refuses a block this account may not edit, as a
/// claim of it would be refused. `told` hears what the server says of who hosts it, each time
/// that changes. It sleeps until woken: by the stack handing over a change, by the app, or by
/// `wake`, which whoever sets `stop` calls.
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
    // The account's access, once the stack knows the block.
    let access = loop {
        if stop() {
            be_host_stack::close(block);
            return Ok(());
        }
        if let Some(node) = be_host_stack::node(block) {
            break node.access;
        }
        headless.run_until(LONGEST_SLEEP, || {
            (stop() || be_host_stack::node(block).is_some()).then_some(())
        });
    };
    if !access.can_edit() {
        be_host_stack::close(block);
        return Err(format!(
            "hosting a block needs Edit on it, and this account has {access:?} on {block}"
        ));
    }
    let editor_block = be_host_runtime::EditorBlock {
        id: block,
        block_type,
        view_block: None,
    };
    let catalog = Arc::new(be_host_runtime::AppCatalog::default());
    let mut instances = 0;
    // The component running, and whether its app has opened it yet.
    let mut running: Option<(EditorInstanceId, bool)> = None;
    let mut said = None;
    let mut failed = None;
    loop {
        be_host_stack::take_updates_in_frame();
        // What the server says now, not what was asked of it: hosting once it names this device.
        let saying = match (
            be_host_stack::hosted().contains(&block),
            be_host_stack::session_host(block),
        ) {
            (true, _) => "hosting",
            (false, Some(Some(false))) => "another device hosts it; standing by",
            _ => "standing by until no one else hosts it",
        };
        if said != Some(saying) {
            told(saying);
            said = Some(saying);
        }
        if stop() {
            break;
        }
        let may_host = be_host_stack::may_host(block);
        match running {
            None if may_host => {
                instances += 1;
                be_host_stack::restart_host(block);
                running = Some((EditorInstanceId(instances), false));
            }
            // Another device hosts it: the component stops, and its claim stays in the session
            // with no release_host, so once that device goes the server names this one host
            // again and the component starts again here, which `be host` always wants. The
            // stack still counts a component running here, so a reconnect that answers no one
            // hosts claims again while it stands by, until the next step starts one.
            Some((instance, _)) if !may_host => {
                be_host_runtime::close(&app.identity.id, instance);
                running = None;
            }
            _ => {}
        }
        let revision = be_host_stack::lease_revision();
        // Reported, as be-host reports a component each frame, until its app opens it; it sleeps
        // until the stack, the app or a stop wakes it. Each step takes what the stack handed
        // over first (`Headless::step` begins a host frame, `begin_host_frame`), so the lease
        // revision read here is current.
        headless.run_until(LONGEST_SLEEP, || {
            if let Some((instance, false)) = running {
                match be_host_runtime::host(&app, &catalog, client, instance, host, editor_block) {
                    be_host_runtime::CreationState::Ready => running = Some((instance, true)),
                    be_host_runtime::CreationState::Failed(error) => {
                        failed = Some(error);
                        return Some(());
                    }
                    _ => {}
                }
            }
            (stop() || be_host_stack::lease_revision() != revision).then_some(())
        });
        if let Some(error) = failed.take() {
            be_host_stack::close(block);
            return Err(format!(
                "{}'s host component could not start: {error}",
                app.identity.id
            ));
        }
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
