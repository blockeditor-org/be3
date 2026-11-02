use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::net::{Http, get};

pub(crate) const HELPER: &str = "https://api.be3.pfg.pw/build";
pub(crate) const PUBLIC: &str = "https://be3-ci.b-cdn.net";
const GATEWAY_HEADER: &str = "Be3-Gateway-Authorization";
const KEY_VARIABLE: &str = "BE3_AUTHORIZATION";
const KEY_FILE: &str = "gateway-authorization";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Product {
    App,
    #[cfg(not(target_os = "android"))]
    Demo,
    #[cfg(target_os = "android")]
    Launcher,
}

impl Product {
    pub(crate) fn name(self) -> String {
        match self {
            Product::App if cfg!(target_os = "android") => "android".to_owned(),
            Product::App => format!("native-{}", platform()),
            #[cfg(not(target_os = "android"))]
            Product::Demo => format!("beui-demo-{}", platform()),
            #[cfg(target_os = "android")]
            Product::Launcher => "launcher-android".to_owned(),
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Product::App => "the app",
            #[cfg(not(target_os = "android"))]
            Product::Demo => "beui's demo",
            #[cfg(target_os = "android")]
            Product::Launcher => "its launcher",
        }
    }

    pub(crate) fn executable(self) -> String {
        let name = match self {
            Product::App if cfg!(target_os = "android") => return "block-app.apk".to_owned(),
            #[cfg(target_os = "android")]
            Product::Launcher => return "be-launcher.apk".to_owned(),
            Product::App => "block-app",
            #[cfg(not(target_os = "android"))]
            Product::Demo => "beui-demo",
        };
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    }
}

pub(crate) fn platform() -> &'static str {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    };
    let cpu = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    match (os, cpu) {
        ("macos", "arm64") => "macos_arm64",
        ("macos", _) => "macos_x86_64",
        ("windows", "arm64") => "windows_arm64",
        ("windows", _) => "windows_x86_64",
        (_, "arm64") => "linux_arm64",
        _ => "linux_x86_64",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum State {
    Queued,
    Running,
    Succeeded,
    Failed,
    Refused,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Status {
    pub(crate) state: State,
    #[serde(default)]
    pub(crate) position: Option<u32>,
    #[serde(default)]
    pub(crate) manifest: Option<String>,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct File {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
    #[serde(default)]
    pub(crate) executable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) product: String,
    pub(crate) commit: String,
    pub(crate) files: Vec<File>,
}

impl Manifest {
    pub(crate) fn parse(document: &[u8]) -> Result<Self, String> {
        let manifest: Manifest = serde_json::from_slice(document)
            .map_err(|error| format!("the build helper described the build as {error}"))?;
        for file in &manifest.files {
            let plain = !file.path.is_empty()
                && !file.path.starts_with('/')
                && file
                    .path
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..");
            let hashed =
                file.sha256.len() == 64 && file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
            if !plain || !hashed {
                return Err(format!(
                    "the build helper named a file {} {}",
                    file.path, file.sha256
                ));
            }
        }
        Ok(manifest)
    }
}

pub(crate) fn file_url(sha256: &str) -> String {
    format!("{PUBLIC}/files/{sha256}")
}

pub(crate) struct Helper<'a> {
    pub(crate) http: &'a mut dyn Http,
    pub(crate) key: String,
}

impl Helper<'_> {
    fn url(product: Product, commit: &str) -> String {
        format!("{HELPER}/products/{}/{commit}", product.name())
    }

    fn call(&mut self, method: &str, url: &str) -> Result<(u16, Vec<u8>), String> {
        let mut body = Vec::new();
        let status = self.http.send(
            method,
            url,
            &[(GATEWAY_HEADER, self.key.as_str())],
            &mut body,
        )?;
        if status == 401 || status == 403 {
            return Err(UNAUTHORIZED.to_owned());
        }
        Ok((status, body))
    }

    fn status_from(&mut self, method: &str, url: &str) -> Result<Status, String> {
        let (code, body) = self.call(method, url)?;
        serde_json::from_slice::<Status>(&body).map_err(|_| {
            format!(
                "the build helper answered {code}: {}",
                String::from_utf8_lossy(&body).trim()
            )
        })
    }

    pub(crate) fn request(&mut self, product: Product, commit: &str) -> Result<Status, String> {
        self.status_from("POST", &Self::url(product, commit))
    }

    pub(crate) fn status(&mut self, product: Product, commit: &str) -> Result<Status, String> {
        self.status_from("GET", &Self::url(product, commit))
    }

    pub(crate) fn log(
        &mut self,
        product: Product,
        commit: &str,
        from: usize,
    ) -> Result<Vec<u8>, String> {
        let url = format!("{}/log?from={from}", Self::url(product, commit));
        let (code, body) = self.call("GET", &url)?;
        if code != 200 {
            return Err(format!("the build helper answered {code} for the log"));
        }
        Ok(body)
    }
}

pub(crate) const UNAUTHORIZED: &str = "The build helper refused the gateway key.";
pub(crate) const NO_KEY: &str = "Enter the API gateway's key to build.";

pub(crate) fn manifest(http: &mut dyn Http, url: &str) -> Result<Manifest, String> {
    let (code, body) = get(http, url, &[])?;
    if code != 200 {
        return Err(format!("{url} answered {code}"));
    }
    Manifest::parse(&body)
}

pub(crate) fn key(files: &Path) -> Option<String> {
    std::env::var(KEY_VARIABLE)
        .ok()
        .into_iter()
        .chain(
            key_files(files)
                .into_iter()
                .filter_map(|path| std::fs::read_to_string(path).ok()),
        )
        .map(|key| key.trim().to_owned())
        .find(|key| !key.is_empty())
}

pub(crate) fn save_key(files: &Path, key: &str) -> Result<(), String> {
    let path = key_files(files)
        .into_iter()
        .next_back()
        .ok_or("there is nowhere to keep the key")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, key.trim()).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(target_os = "android")]
fn key_files(files: &Path) -> Vec<PathBuf> {
    vec![files.join(KEY_FILE)]
}

#[cfg(not(target_os = "android"))]
fn key_files(_files: &Path) -> Vec<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from));
    config
        .map(|config| vec![config.join("be3").join(KEY_FILE)])
        .unwrap_or_default()
}
