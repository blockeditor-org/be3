mod apps;
mod client;

use std::{cell::RefCell, collections::HashMap};

use be_sdk_api::HostPanel;

use crate::ui::{DebugCommand, DebugView};

thread_local! {
    static SHOWN: RefCell<HashMap<HostPanel, usize>> = RefCell::new(HashMap::new());
}

fn is_shown(panel: HostPanel) -> bool {
    SHOWN.with(|shown| shown.borrow().get(&panel).is_some_and(|count| *count > 0))
}

pub(crate) fn panel_shown(panel: HostPanel) {
    SHOWN.with(|shown| *shown.borrow_mut().entry(panel).or_default() += 1);
    be_host_base::host::request_repaint();
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
        DebugCommand::KillApp(app_id) => be_host_runtime::kill(&app_id),
        DebugCommand::CrashApp(app_id) => {
            be_host_runtime::crash(&app_id, "It was crashed from the Apps panel.".to_owned())
        }
    }
}

pub(crate) fn view() -> DebugView {
    DebugView {
        client: is_shown(HostPanel::BlockStack).then(client::lines),
        performance: is_shown(HostPanel::Performance).then(be_host_base::performance::rows),
        apps: is_shown(HostPanel::Apps).then(apps::view),
    }
}
