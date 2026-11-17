use super::*;

mod a_file_the_release_does_not_list_fails;
mod a_list_of_hashes_other_than_the_one_be_ships_with_is_refused;
mod a_release_file_is_downloaded_once_then_read_from_the_cache;
mod a_release_file_that_does_not_match_its_hash_fails_naming_it;
mod nothing_is_downloaded_until_an_app_file_is_read;

/// A release at `https://mirror.test/data/build/web` holding `files`, which counts the fetches
/// of each URL, its hashes listed as `listed` gives them, and `apps` naming that list as a
/// deployed `be`'s `build-info.json` does.
struct Fake {
    files: HashMap<String, Vec<u8>>,
    fetched: Mutex<Vec<String>>,
    apps: ReleaseApps,
}

const BASE: &str = "https://mirror.test/data/build/web";

impl Fake {
    fn new(files: &[(&str, &[u8])], listed: &[(&str, &[u8])]) -> Self {
        let mut held: HashMap<String, Vec<u8>> = files
            .iter()
            .map(|(path, bytes)| (format!("{BASE}/{path}"), bytes.to_vec()))
            .collect();
        let hashes: HashMap<&str, String> = listed
            .iter()
            .map(|(path, bytes)| (*path, sha256(bytes)))
            .collect();
        let hashes = serde_json::to_vec(&hashes).expect("the hashes serialize");
        let apps = ReleaseApps {
            url: BASE.to_owned(),
            sha256: sha256(&hashes),
        };
        held.insert(format!("{BASE}/{HASHES}"), hashes);
        Self {
            files: held,
            fetched: Mutex::default(),
            apps,
        }
    }

    fn release(&self, cache: PathBuf) -> Release<impl Fn(&str) -> Result<Vec<u8>, String> + '_> {
        Release::new(self.apps.clone(), cache, |url: &str| self.fetch(url))
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
    let directory = std::env::temp_dir().join(format!("be-release-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("the cache is made");
    directory
}
