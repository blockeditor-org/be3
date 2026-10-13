use super::{INDEX, Plugins};

pub(super) fn scan() -> Plugins {
    if let Ok(id) = read(build_id::FILE) {
        build_id::install(&String::from_utf8_lossy(&id));
    }
    let mut plugins = Plugins::default();
    let manifests = read(INDEX).and_then(|document| {
        serde_json::from_slice::<Vec<String>>(&document).map_err(|error| error.to_string())
    });
    match manifests {
        Ok(manifests) => {
            for manifest in manifests {
                match read(&manifest)
                    .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
                {
                    Ok(document) => plugins.add(&manifest, &document),
                    Err(error) => plugins.error(&manifest, error),
                }
            }
        }
        Err(error) => plugins.error(INDEX, error),
    }
    plugins
}

#[cfg(not(target_os = "android"))]
pub(crate) fn read(path: &str) -> Result<Vec<u8>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let root = executable
        .parent()
        .ok_or("the executable is in no directory")?;
    std::fs::read(root.join(path)).map_err(|error| error.to_string())
}

#[cfg(target_os = "android")]
static ASSETS: std::sync::OnceLock<beui::AndroidApp> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub(crate) fn use_assets(app: beui::AndroidApp) {
    let _ = ASSETS.set(app);
}

#[cfg(target_os = "android")]
pub(crate) fn read(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let app = ASSETS.get().ok_or("the app's assets were not given")?;
    let name =
        std::ffi::CString::new(path).map_err(|_| "the asset path is not a C string".to_owned())?;
    let mut asset = app.asset_manager().open(&name).ok_or("no such asset")?;
    let mut bytes = Vec::new();
    asset
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}
