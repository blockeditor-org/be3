use super::Apps;

pub async fn load() {
    let apps = super::discover(|path| be_host_base::platform::http::get(path, Vec::new())).await;
    super::install(apps);
}

pub(super) fn scan() -> Apps {
    Apps::default()
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    be_host_base::platform::http::fetch(path, Vec::new(), Default::default(), deliver);
}
