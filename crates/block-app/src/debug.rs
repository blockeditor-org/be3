mod client;
mod plugins;

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
    SHOWN.with(|shown| *shown.borrow_mut().entry(panel).or_default() += 1);
    block_app_base::host::request_repaint();
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

pub(crate) fn command(command: DebugCommand) {
    match command {
        DebugCommand::KillPlugin(plugin_id) => block_app_plugin_host::kill(&plugin_id),
        DebugCommand::CrashPlugin(plugin_id) => block_app_plugin_host::crash(
            &plugin_id,
            "It was crashed from the Plugins panel.".to_owned(),
        ),
    }
}

pub(crate) fn view() -> DebugView {
    DebugView {
        client: is_shown(HostPanel::BlockStack).then(client::lines),
        performance: is_shown(HostPanel::Performance).then(block_app_base::performance::rows),
        plugins: is_shown(HostPanel::Plugins).then(plugins::view),
    }
}
