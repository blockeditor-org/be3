use super::Plugins;

pub(crate) async fn load() {
    let plugins = super::discover(|path| block_app_base::platform::http::get(path, Vec::new())).await;
    super::install(plugins);
}

pub(super) fn scan() -> Plugins {
    Plugins::default()
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    block_app_base::platform::http::fetch(path, Vec::new(), Default::default(), deliver);
}
