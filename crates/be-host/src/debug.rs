mod apps;
mod client;

pub(crate) use apps::bytes;

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
    watch_memory();
    be_host_base::host::request_repaint();
}

fn watch_memory() {
    #[cfg(target_os = "linux")]
    crate::memory::watch(is_shown(HostPanel::Performance));
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
    watch_memory();
}

pub(crate) fn command(command: DebugCommand) {
    match command {
        DebugCommand::KillApp(app_id) => be_host_runtime::kill(&app_id),
        DebugCommand::CrashApp(app_id) => be_host_runtime::crash(
            &app_id,
            "It was crashed from be-host's Running Apps panel.".to_owned(),
        ),
    }
}

pub(crate) fn view() -> DebugView {
    DebugView {
        client: is_shown(HostPanel::BlockStack).then(client::lines),
        performance: is_shown(HostPanel::Performance).then(be_host_base::performance::rows),
        #[cfg(target_os = "linux")]
        memory: is_shown(HostPanel::Performance)
            .then(crate::memory::view)
            .flatten(),
        #[cfg(not(target_os = "linux"))]
        memory: None,
        apps: is_shown(HostPanel::Apps).then(apps::view),
    }
}
