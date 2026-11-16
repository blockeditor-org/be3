use super::*;

mod a_release_file_is_downloaded_once_then_read_from_the_cache;
mod a_release_file_that_does_not_match_its_hash_fails_naming_it;
mod a_file_the_release_does_not_list_fails;

/// A release at `https://mirror.test/data/build/web` holding `files`, which counts the fetches
/// of each URL, its hashes listed as `listed` gives them.
struct Fake {
    files: HashMap<String, Vec<u8>>,
    fetched: Mutex<Vec<String>>,
}

const BASE: &str = "https://mirror.test/data/build/web";

impl Fake {
    fn new(files: &[(&str, &[u8])], listed: &[(&str, &[u8])]) -> Self {
        let mut held: HashMap<String, Vec<u8>> = files
            .iter()
            .map(|(path, bytes)| (format!("{BASE}/{path}"), bytes.to_vec()))
            .collect();
        let hashes: HashMap<&str, String> =
            listed.iter().map(|(path, bytes)| (*path, sha256(bytes))).collect();
        held.insert(
            format!("{BASE}/{HASHES}"),
            serde_json::to_vec(&hashes).expect("the hashes serialize"),
        );
        Self {
            files: held,
            fetched: Mutex::default(),
        }
    }

    fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        self.fetched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.to_owned());
        self.files.get(url).cloned().ok_or_else(|| "404".to_owned())
    }

    fn fetches_of(&self, path: &str) -> usize {
        let url = format!("{BASE}/{path}");
        self.fetched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|fetched| **fetched == url)
            .count()
    }
}

fn cache() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("be-host-apps-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("the cache is made");
    directory
}
