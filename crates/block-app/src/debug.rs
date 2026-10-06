mod client;
mod plugins;
pub(crate) mod version;

use std::{cell::RefCell, collections::HashMap};

use block_plugin_api::HostPanel;

use crate::ui::{DebugCommand, DebugView};

thread_local! {
    static SHOWN: RefCell<HashMap<HostPanel, usize>> = RefCell::new(HashMap::new());
}

fn is_shown(panel: HostPanel) -> bool {
    SHOWN.with(|shown| shown.borrow().get(&panel).is_some_and(|count| *count > 0))
}

pub(crate) fn panel_shown(panel: HostPanel) {
    let first = SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        let count = shown.entry(panel).or_default();
        *count += 1;
        *count == 1
    });
    if first && panel == HostPanel::Version {
        version::open();
    }
    crate::host::request_repaint();
}

pub(crate) fn panel_hidden(panel: HostPanel) {
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        if let Some(count) = shown.get_mut(&panel) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                shown.remove(&panel);
            }
        }
    });
}

pub(crate) fn poll() {
    if is_shown(HostPanel::Version) {
        version::poll();
    }
}

pub(crate) fn command(command: DebugCommand) {
    match command {
        DebugCommand::KillPlugin(plugin_id) => crate::plugin_host::kill(&plugin_id),
        DebugCommand::RefreshVersions => version::refresh(),
        DebugCommand::Install(run) => version::install(run),
        DebugCommand::OpenUrl(url) => crate::platform::open_url(&url),
    }
}

pub(crate) fn view() -> DebugView {
    DebugView {
        client: is_shown(HostPanel::BlockStack).then(client::lines),
        performance: is_shown(HostPanel::Performance).then(crate::performance::rows),
        plugins: is_shown(HostPanel::Plugins).then(plugins::view),
        version: is_shown(HostPanel::Version).then(version::view),
    }
}
