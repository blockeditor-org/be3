use super::Plugins;

pub async fn load() {
    let plugins = super::discover(|path| be_host_base::platform::http::get(path, Vec::new())).await;
    super::install(plugins);
}

pub(super) fn scan() -> Plugins {
    Plugins::default()
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    be_host_base::platform::http::fetch(path, Vec::new(), Default::default(), deliver);
}
