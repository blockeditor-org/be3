//! The build helper's wire (guides/build_server.md): the products it builds, the status it
//! answers for one, and the manifest it publishes for a build that succeeded. be-tool-build-helper
//! writes these and be-launcher reads them.

use serde::{Deserialize, Serialize};

pub const PLATFORMS: [&str; 6] = [
    "linux_x86_64",
    "linux_arm64",
    "macos_x86_64",
    "macos_arm64",
    "windows_x86_64",
    "windows_arm64",
];

/// The platform of [`PLATFORMS`] this program was built for, so the one whose native builds it
/// can run.
pub fn platform() -> &'static str {
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

/// A product the helper builds, named in its URLs by [`Product::name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    /// be-host's `:android-dist`.
    Android,
    /// be-launcher's `:android-dist`.
    LauncherAndroid,
    /// A release build of be-host for one of [`PLATFORMS`].
    Native(&'static str),
    /// A release build of beui-demo for one of [`PLATFORMS`].
    BeuiDemo(&'static str),
}

impl Product {
    pub fn parse(name: &str) -> Option<Self> {
        let platform = |prefix: &str| {
            let platform = name.strip_prefix(prefix)?;
            PLATFORMS.into_iter().find(|known| *known == platform)
        };
        match name {
            "android" => Some(Self::Android),
            "launcher-android" => Some(Self::LauncherAndroid),
            _ => platform("native-")
                .map(Self::Native)
                .or_else(|| platform("beui-demo-").map(Self::BeuiDemo)),
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Android => "android".into(),
            Self::LauncherAndroid => "launcher-android".into(),
            Self::Native(platform) => format!("native-{platform}"),
            Self::BeuiDemo(platform) => format!("beui-demo-{platform}"),
        }
    }
}

/// Whether `text` is a full lowercase commit SHA, the only way the helper takes a commit.
pub fn is_commit(text: &str) -> bool {
    text.len() == 40
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub fn finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

/// What `POST` and `GET /build/products/PRODUCT/COMMIT` answer with 200 for a build the helper
/// knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub product: String,
    pub commit: String,
    pub state: State,
    /// How many builds are ahead of a queued one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    /// The URL of a succeeded build's [`Manifest`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
    /// Why a build failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What the helper answers in place of a [`Status`]: with 404 when it knows no such product,
/// commit or build, and with 422 when it will not build the commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A published build: its files, each stored once by content hash at [`file_url`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub product: String,
    pub commit: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Where the file goes inside the build, relative and `/`-separated.
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub executable: bool,
}

/// Where the CDN at `public` serves the manifest of `product` built at `commit`.
pub fn manifest_url(public: &str, product: &str, commit: &str) -> String {
    format!(
        "{}/builds/{product}/{commit}.json",
        public.trim_end_matches('/')
    )
}

/// Where the CDN at `public` serves the file whose content hashes to `sha256`.
pub fn file_url(public: &str, sha256: &str) -> String {
    format!("{}/files/{sha256}", public.trim_end_matches('/'))
}

#[cfg(test)]
mod tests;
