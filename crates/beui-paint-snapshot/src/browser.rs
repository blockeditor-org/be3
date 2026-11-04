use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const MAGIC: &str = "BE3BROWSER 1\n";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSnapshot {
    pub size: [u32; 2],
    pub document: Vec<String>,
    pub files: BTreeMap<String, BrowserFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserFile {
    pub content_type: String,
    pub text: Option<Vec<String>>,
    pub bytes: Option<Vec<u8>>,
}

impl BrowserSnapshot {
    pub fn new(size: [u32; 2], document: &str) -> Self {
        Self {
            size,
            document: document.split_inclusive('>').map(str::to_owned).collect(),
            files: BTreeMap::new(),
        }
    }

    pub fn file(mut self, url: &str, content_type: &str, bytes: &[u8]) -> Self {
        let (text, binary) = match std::str::from_utf8(bytes) {
            Ok(text) => (
                Some(text.split_inclusive('\n').map(str::to_owned).collect()),
                None,
            ),
            Err(_) => (None, Some(bytes.to_vec())),
        };
        self.files.insert(
            url.to_owned(),
            BrowserFile {
                content_type: content_type.to_owned(),
                text,
                bytes: binary,
            },
        );
        self
    }

    pub fn is_browser(bytes: &[u8]) -> bool {
        bytes.starts_with(b"BE3BROWSER ")
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        Ok(format!("{MAGIC}{json}\n").into_bytes())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = bytes
            .strip_prefix(MAGIC.as_bytes())
            .ok_or("unsupported browser painting")?;
        let snapshot: Self = serde_json::from_slice(json).map_err(|error| error.to_string())?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), String> {
        if self.size.contains(&0) {
            return Err("browser painting dimensions must be positive".into());
        }
        for (url, file) in &self.files {
            if !url.starts_with('/') && !url.starts_with("https://") && !url.starts_with("http://")
            {
                return Err(format!(
                    "embedded file must have an absolute path or URL: {url}"
                ));
            }
            if file.text.is_some() == file.bytes.is_some() {
                return Err(format!(
                    "embedded file must hold either text or bytes: {url}"
                ));
            }
        }
        Ok(())
    }

    #[track_caller]
    pub fn snapshot(&self, name: &str) {
        assert!(
            name.ends_with(".browser"),
            "browser painting names must end with .browser"
        );
        let bytes = self
            .encode()
            .expect("the browser painting could not be encoded");
        crate::accepted::assert_bytes(
            name,
            "UPDATE_SNAPSHOTS=1 ./scripts/test TEST_NAME",
            "push it and review the browser painting in the pull request's text diff and painting comment",
            |previous| previous == bytes,
            || bytes.clone(),
            |previous| {
                let previous = String::from_utf8_lossy(previous);
                let current = String::from_utf8_lossy(&bytes);
                let old: Vec<_> = previous.lines().collect();
                let new: Vec<_> = current.lines().collect();
                let first = old
                    .iter()
                    .zip(&new)
                    .position(|(a, b)| a != b)
                    .unwrap_or(old.len().min(new.len()));
                format!(
                    "browser document or embedded files changed at line {}\n- {}\n+ {}",
                    first + 1,
                    old.get(first).unwrap_or(&"<end>"),
                    new.get(first).unwrap_or(&"<end>")
                )
            },
        );
    }
}
