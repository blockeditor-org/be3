use serde::{Deserialize, Serialize};

pub const PLATFORMS: [&str; 6] = [
    "linux_x86_64",
    "linux_arm64",
    "macos_x86_64",
    "macos_arm64",
    "windows_x86_64",
    "windows_arm64",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    Android,
    LauncherAndroid,
    Native(&'static str),
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

#[derive(Debug, Serialize)]
pub struct Status {
    pub product: String,
    pub commit: String,
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub product: String,
    pub commit: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub executable: bool,
}

pub fn manifest_url(public: &str, product: &str, commit: &str) -> String {
    format!(
        "{}/builds/{product}/{commit}.json",
        public.trim_end_matches('/')
    )
}
