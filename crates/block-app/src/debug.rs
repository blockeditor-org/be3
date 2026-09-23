mod client;
mod network;
mod plugins;
pub(crate) mod version;

#[cfg(feature = "terminal")]
mod terminal;

use std::{cell::RefCell, collections::HashSet};

use block_client::BlockClient;

use crate::ui::{DebugCommand, DebugView, DebugWindow};

thread_local! {
    static OPEN: RefCell<HashSet<DebugWindow>> = RefCell::new(HashSet::new());
}

fn is_open(window: DebugWindow) -> bool {
    OPEN.with(|open| open.borrow().contains(&window))
}

fn set_open(window: DebugWindow, shown: bool) {
    OPEN.with(|open| {
        let mut open = open.borrow_mut();
        match shown {
            true => open.insert(window),
            false => open.remove(&window),
        };
    });
}

pub(crate) fn close_client_windows() {
    set_open(DebugWindow::Client, false);
    set_open(DebugWindow::Network, false);
}

pub(crate) fn poll(client: &BlockClient) {
    if is_open(DebugWindow::Network) {
        client.enable_network_traffic_logging();
    }
    if is_open(DebugWindow::Version) {
        version::poll();
    }
    #[cfg(feature = "terminal")]
    if is_open(DebugWindow::Terminal) {
        terminal::poll();
    }
}

pub(crate) fn command(client: &BlockClient, command: DebugCommand) {
    match command {
        DebugCommand::Open(window) => {
            set_open(window, true);
            if window == DebugWindow::Version {
                version::open();
            }
        }
        DebugCommand::Close(window) => {
            set_open(window, false);
            #[cfg(feature = "terminal")]
            if window == DebugWindow::Terminal {
                terminal::close();
            }
        }
        DebugCommand::PauseSending => client.pause_sending(),
        DebugCommand::StepSending => client.step_sending(),
        DebugCommand::ResumeSending => client.resume_sending(),
        DebugCommand::ClearTraffic => client.clear_network_traffic(),
        DebugCommand::KillPlugin(plugin_id) => crate::plugin_host::kill(&plugin_id),
        DebugCommand::RefreshVersions => version::refresh(),
        DebugCommand::Install(run) => version::install(run),
        DebugCommand::OpenUrl(url) => crate::platform::open_url(&url),
        #[cfg(feature = "terminal")]
        DebugCommand::Terminal(input) => terminal::input(input),
        #[cfg(not(feature = "terminal"))]
        DebugCommand::Terminal(_) => {}
    }
}

pub(crate) fn view(client: &BlockClient) -> DebugView {
    DebugView {
        client: is_open(DebugWindow::Client).then(|| client::lines(client)),
        network: is_open(DebugWindow::Network).then(|| network::view(client)),
        performance: is_open(DebugWindow::Performance).then(crate::performance::rows),
        plugins: is_open(DebugWindow::Plugins).then(plugins::view),
        version: is_open(DebugWindow::Version).then(version::view),
        #[cfg(feature = "terminal")]
        terminal: is_open(DebugWindow::Terminal).then(terminal::view),
        #[cfg(not(feature = "terminal"))]
        terminal: None,
    }
}
