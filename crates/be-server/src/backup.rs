use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use be_store::{FileStore, Hash, ObjectStore};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use rand::TryRngCore;
use rusqlite::Connection;

const SEALED_MAGIC: &[u8] = b"be3-backup-v1";
const NONCE_LEN: usize = 12;
const DATABASES: &str = "databases";
const OBJECTS: &str = "objects";
const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const KEEP_EVERY: u64 = 48 * HOUR;
const KEEP_DAILY: u64 = 30 * DAY;
const KEEP_MONTHLY: u64 = 365 * DAY;

#[derive(Debug)]
pub enum BackupError {
    Io(io::Error),
    Database(rusqlite::Error),
    Store(be_store::StoreError),
    Key,
    Remote(String),
    Missing(String),
    NotEmpty(PathBuf),
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Database(error) => write!(formatter, "the database failed: {error}"),
            Self::Store(error) => write!(formatter, "{error}"),
            Self::Key => formatter
                .write_str("the backup key is not 64 hex characters, or does not open this backup"),
            Self::Remote(message) => write!(formatter, "the backup target failed: {message}"),
            Self::Missing(what) => write!(formatter, "the backup has no {what}"),
            Self::NotEmpty(path) => write!(
                formatter,
                "{} already holds a server; restore into an empty directory",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<io::Error> for BackupError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for BackupError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl From<be_store::StoreError> for BackupError {
    fn from(error: be_store::StoreError) -> Self {
        Self::Store(error)
    }
}

pub trait Target {
    fn name(&self) -> String;

    fn put(&self, path: &str, bytes: &[u8]) -> Result<(), BackupError>;

    fn get(&self, path: &str) -> Result<Option<Vec<u8>>, BackupError>;

    fn list(&self, directory: &str) -> Result<Vec<String>, BackupError>;

    fn delete(&self, path: &str) -> Result<(), BackupError>;
}

pub struct Directory(pub PathBuf);

impl Target for Directory {
    fn name(&self) -> String {
        self.0.display().to_string()
    }

    fn put(&self, path: &str, bytes: &[u8]) -> Result<(), BackupError> {
        let path = self.0.join(path);
        let parent = path.parent().ok_or(BackupError::Key)?;
        fs::create_dir_all(parent)?;
        let partial = path.with_extension("partial");
        fs::write(&partial, bytes)?;
        fs::File::open(&partial)?.sync_all()?;
        fs::rename(&partial, &path)?;
        Ok(())
    }

    fn get(&self, path: &str) -> Result<Option<Vec<u8>>, BackupError> {
        match fs::read(self.0.join(path)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn list(&self, directory: &str) -> Result<Vec<String>, BackupError> {
        match fs::read_dir(self.0.join(directory)) {
            Ok(entries) => Ok(entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| !name.ends_with(".partial"))
                .collect()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    fn delete(&self, path: &str) -> Result<(), BackupError> {
        match fs::remove_file(self.0.join(path)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

pub struct Bunny {
    base: String,
    key: String,
    agent: ureq::Agent,
}

impl Bunny {
    pub fn new(endpoint: &str, zone: &str, key: String) -> Self {
        Self {
            base: format!("{}/{zone}", endpoint.trim_end_matches('/')),
            key,
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(120))
                .build(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }
}

fn remote(error: ureq::Error) -> BackupError {
    BackupError::Remote(error.to_string())
}

#[derive(serde::Deserialize)]
struct Listed {
    #[serde(rename = "ObjectName")]
    name: String,
    #[serde(rename = "IsDirectory")]
    directory: bool,
}

impl Target for Bunny {
    fn name(&self) -> String {
        self.base.clone()
    }

    fn put(&self, path: &str, bytes: &[u8]) -> Result<(), BackupError> {
        self.agent
            .put(&self.url(path))
            .set("AccessKey", &self.key)
            .set("Content-Type", "application/octet-stream")
            .send_bytes(bytes)
            .map_err(remote)?;
        Ok(())
    }

    fn get(&self, path: &str) -> Result<Option<Vec<u8>>, BackupError> {
        match self
            .agent
            .get(&self.url(path))
            .set("AccessKey", &self.key)
            .call()
        {
            Ok(response) => {
                let mut bytes = Vec::new();
                io::Read::read_to_end(&mut response.into_reader(), &mut bytes)?;
                Ok(Some(bytes))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(error) => Err(remote(error)),
        }
    }

    fn list(&self, directory: &str) -> Result<Vec<String>, BackupError> {
        match self
            .agent
            .get(&format!("{}/", self.url(directory)))
            .set("AccessKey", &self.key)
            .set("Accept", "application/json")
            .call()
        {
            Ok(response) => {
                let listed: Vec<Listed> = response
                    .into_json()
                    .map_err(|error| BackupError::Remote(error.to_string()))?;
                Ok(listed
                    .into_iter()
                    .filter(|entry| !entry.directory)
                    .map(|entry| entry.name)
                    .collect())
            }
            Err(ureq::Error::Status(404, _)) => Ok(Vec::new()),
            Err(error) => Err(remote(error)),
        }
    }

    fn delete(&self, path: &str) -> Result<(), BackupError> {
        match self
            .agent
            .delete(&self.url(path))
            .set("AccessKey", &self.key)
            .call()
        {
            Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(error) => Err(remote(error)),
        }
    }
}

pub struct BackupKey([u8; 32]);

impl BackupKey {
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut key)
            .expect("the operating system has entropy");
        Self(key)
    }

    pub fn to_hex(&self) -> String {
        Hash::from_bytes(self.0).to_hex()
    }

    pub fn from_hex(text: &str) -> Result<Self, BackupError> {
        Hash::from_hex(text.trim())
            .map(|hash| Self(*hash.as_bytes()))
            .ok_or(BackupError::Key)
    }

    fn cipher(&self) -> ChaCha20Poly1305 {
        ChaCha20Poly1305::new(Key::from_slice(&self.0))
    }

    fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .expect("the operating system has entropy");
        let ciphertext = self
            .cipher()
            .encrypt(Nonce::from_slice(&nonce), plain)
            .expect("chacha20poly1305 never fails on a valid key and nonce");
        [SEALED_MAGIC, &nonce, &ciphertext].concat()
    }

    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, BackupError> {
        let rest = sealed.strip_prefix(SEALED_MAGIC).ok_or(BackupError::Key)?;
        let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN).ok_or(BackupError::Key)?;
        self.cipher()
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| BackupError::Key)
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct BackupReport {
    pub snapshot: String,
    pub uploaded: usize,
    pub objects: usize,
    pub pruned: usize,
}

pub fn backup(
    data_dir: &Path,
    target: &dyn Target,
    key: &BackupKey,
    now: SystemTime,
) -> Result<BackupReport, BackupError> {
    let staging = data_dir.join("backup.sqlite");
    let _ = fs::remove_file(&staging);
    Connection::open(data_dir.join("metadata.sqlite"))?
        .execute("VACUUM INTO ?1", [staging.to_string_lossy()])?;
    let snapshot = fs::read(&staging);
    let referenced = referenced_objects(&Connection::open(&staging)?);
    fs::remove_file(&staging)?;
    let (snapshot, referenced) = (snapshot?, referenced?);

    let objects = FileStore::open(data_dir.join("objects"))?;
    let record = data_dir.join(format!(
        "backed-up-{}.txt",
        Hash::of(target.name().as_bytes()).to_hex()
    ));
    let mut uploaded: HashSet<String> = fs::read_to_string(&record)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut sent = 0;
    for hash in &referenced {
        let hex = hash.to_hex();
        if uploaded.contains(&hex) {
            continue;
        }
        let bytes = objects
            .get(*hash)?
            .ok_or_else(|| BackupError::Missing(format!("object {hex} on this server")))?;
        target.put(&object_path(&hex), &bytes)?;
        uploaded.insert(hex);
        sent += 1;
    }
    let mut listed: Vec<&String> = uploaded.iter().collect();
    listed.sort();
    let written: String = listed.iter().map(|hex| format!("{hex}\n")).collect();
    fs::write(&record, written)?;

    let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let name = format!("{}.sealed", timestamp(seconds));
    target.put(&format!("{DATABASES}/{name}"), &key.seal(&snapshot))?;
    let pruned = prune(target, seconds)?;
    Ok(BackupReport {
        snapshot: name,
        uploaded: sent,
        objects: referenced.len(),
        pruned,
    })
}

fn object_path(hex: &str) -> String {
    format!("{OBJECTS}/{}/{}", &hex[..2], &hex[2..])
}

fn referenced_objects(database: &Connection) -> Result<Vec<Hash>, BackupError> {
    let mut statement = database.prepare("SELECT hash FROM object_refs WHERE count > 0")?;
    let hashes = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    hashes
        .iter()
        .map(|hex| Hash::from_hex(hex).ok_or(BackupError::Missing(format!("a valid hash: {hex}"))))
        .collect()
}

pub(crate) fn prune(target: &dyn Target, now: u64) -> Result<usize, BackupError> {
    let mut snapshots: Vec<(u64, String)> = target
        .list(DATABASES)?
        .into_iter()
        .filter_map(|name| Some((parse_timestamp(name.strip_suffix(".sealed")?)?, name)))
        .collect();
    snapshots.sort_by(|left, right| right.cmp(left));
    let mut days = HashSet::new();
    let mut months = HashSet::new();
    let mut pruned = 0;
    for (index, (taken, name)) in snapshots.iter().enumerate() {
        let age = now.saturating_sub(*taken);
        let (year, month, day) = civil(*taken / DAY);
        let keep = index == 0
            || age < KEEP_EVERY
            || (age < KEEP_DAILY && days.insert((year, month, day)))
            || (age < KEEP_MONTHLY && months.insert((year, month)));
        days.insert((year, month, day));
        months.insert((year, month));
        if !keep {
            target.delete(&format!("{DATABASES}/{name}"))?;
            pruned += 1;
        }
    }
    Ok(pruned)
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct RestoreReport {
    pub snapshot: String,
    pub objects: usize,
}

pub fn restore(
    target: &dyn Target,
    key: &BackupKey,
    data_dir: &Path,
    snapshot: Option<&str>,
) -> Result<RestoreReport, BackupError> {
    if data_dir.join("metadata.sqlite").exists() {
        return Err(BackupError::NotEmpty(data_dir.to_path_buf()));
    }
    let name = match snapshot {
        Some(name) => name.to_owned(),
        None => target
            .list(DATABASES)?
            .into_iter()
            .filter(|name| name.ends_with(".sealed"))
            .max()
            .ok_or_else(|| BackupError::Missing("database snapshot".to_owned()))?,
    };
    let sealed = target
        .get(&format!("{DATABASES}/{name}"))?
        .ok_or_else(|| BackupError::Missing(format!("snapshot {name}")))?;
    let database = key.open(&sealed)?;
    fs::create_dir_all(data_dir)?;
    let objects = FileStore::open(data_dir.join("objects"))?;
    let staging = data_dir.join("restoring.sqlite");
    fs::write(&staging, &database)?;
    let referenced = referenced_objects(&Connection::open(&staging)?)?;
    for hash in &referenced {
        let hex = hash.to_hex();
        let bytes = target
            .get(&object_path(&hex))?
            .ok_or_else(|| BackupError::Missing(format!("object {hex}")))?;
        if Hash::of(&bytes) != *hash {
            return Err(BackupError::Missing(format!(
                "an intact copy of object {hex}"
            )));
        }
        objects.put(&bytes)?;
    }
    fs::rename(&staging, data_dir.join("metadata.sqlite"))?;
    Ok(RestoreReport {
        snapshot: name,
        objects: referenced.len(),
    })
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct VerifyReport {
    pub objects: usize,
    pub missing: Vec<String>,
}

pub fn verify(data_dir: &Path) -> Result<VerifyReport, BackupError> {
    let database = Connection::open(data_dir.join("metadata.sqlite"))?;
    let referenced = referenced_objects(&database)?;
    let objects = FileStore::open(data_dir.join("objects"))?;
    let mut missing = Vec::new();
    for hash in &referenced {
        if objects.get(*hash)?.is_none() {
            missing.push(hash.to_hex());
        }
    }
    Ok(VerifyReport {
        objects: referenced.len(),
        missing,
    })
}

pub(crate) fn timestamp(seconds: u64) -> String {
    let (year, month, day) = civil(seconds / DAY);
    let rest = seconds % DAY;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}-{:02}-{:02}Z",
        rest / HOUR,
        rest % HOUR / 60,
        rest % 60
    )
}

fn parse_timestamp(text: &str) -> Option<u64> {
    let (date, time) = text.strip_suffix('Z')?.split_once('T')?;
    let mut date = date.split('-').map(str::parse::<u64>);
    let mut time = time.split('-').map(str::parse::<u64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let (hour, minute, second) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    Some(days_from_civil(year, month, day) * DAY + hour * HOUR + minute * 60 + second)
}

fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
