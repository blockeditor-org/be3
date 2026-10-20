use std::{
    fs,
    io::{Cursor, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    str::FromStr,
    time::Duration,
};

use base64::Engine as _;
use rsa::{
    Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey},
    rand_core::{OsRng, RngCore},
};
use sha2::{Digest, Sha256};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{Decode, Encode},
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
};

const USAGE: &str = "usage: apk-sign --in APK --out APK [--keystore FILE] [--password-file FILE] [--build-info JSON] [--signing dev|ci|release] [--install --activity PACKAGE/ACTIVITY]";

const CHUNK: usize = 1 << 20;
const BLOCK_MAGIC: &[u8; 16] = b"APK Sig Block 42";
const V2_BLOCK: u32 = 0x7109_871a;
const RSA_PKCS1_SHA256: u32 = 0x0103;

pub fn main(arguments: Vec<String>) -> ExitCode {
    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("apk-sign: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let (mut input, mut output, mut key, mut password, mut metadata, mut activity) =
        (None, None, None, None, None, None);
    let mut signing = "dev".to_owned();
    let mut install = false;
    while let Some(argument) = arguments.next() {
        if argument == "--install" {
            install = true;
            continue;
        }
        let value = arguments.next().ok_or(USAGE)?;
        match argument.as_str() {
            "--in" => input = Some(PathBuf::from(value)),
            "--out" => output = Some(PathBuf::from(value)),
            "--keystore" => key = Some(PathBuf::from(value)),
            "--password-file" => password = Some(PathBuf::from(value)),
            "--build-info" => metadata = Some(PathBuf::from(value)),
            "--signing" => signing = value,
            "--activity" => activity = Some(value),
            _ => return Err(USAGE.into()),
        }
    }
    let input = input.ok_or(USAGE)?;
    let output = output.ok_or(USAGE)?;
    if install && activity.is_none() {
        return Err("--install needs --activity PACKAGE/ACTIVITY".into());
    }
    let key = match key {
        Some(key) => key,
        None => match signing.as_str() {
            "dev" => {
                let key = PathBuf::from("target/android-debug.keystore");
                if !key.exists() {
                    generate_key(&key, 2048)?;
                }
                key
            }
            "ci" => ci_key(),
            "release" => release_key()?,
            _ => return Err(USAGE.into()),
        },
    };
    let password = match password {
        Some(path) => {
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            text.lines().next().unwrap_or_default().to_owned()
        }
        None => "android".to_owned(),
    };
    sign(&input, &output, &key, metadata.as_deref(), &password)?;
    if install {
        let adb = adb();
        status(Command::new(&adb).args(["install", "-r"]).arg(&output))?;
        status(Command::new(&adb).args([
            "shell",
            "am",
            "start",
            "-n",
            activity.as_deref().unwrap(),
        ]))?;
    }
    Ok(())
}

fn status(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !status.success() {
        return Err(format!("{command:?}: {status}"));
    }
    Ok(())
}

fn adb() -> PathBuf {
    if let Some(path) = std::env::var_os("ADB") {
        return path.into();
    }
    let name = if cfg!(windows) { "adb.exe" } else { "adb" };
    let mut roots: Vec<PathBuf> = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        roots.extend([
            home.join("Android/Sdk"),
            home.join("Library/Android/sdk"),
            home.join("AppData/Local/Android/Sdk"),
        ]);
    }
    roots
        .into_iter()
        .map(|root| root.join("platform-tools").join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| name.into())
}

pub(crate) fn ci_key() -> PathBuf {
    std::env::var_os("BE3_ANDROID_CI_KEYSTORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| "buck/android/ci-keystore.base64".into())
}

pub(crate) fn release_key() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("BE3_ANDROID_RELEASE_KEYSTORE") {
        return Ok(path.into());
    }
    let config = if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or("HOME or USERPROFILE is required")?,
        )
        .join(".config")
    };
    let saved = config.join("be3/android-release-keystore.base64");
    if saved.is_file() {
        return Ok(saved);
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(format!(
            "no release keystore at {}; set BE3_ANDROID_RELEASE_KEYSTORE or run deploy at a terminal to make or import one",
            saved.display()
        ));
    }
    eprintln!("Android updates must use the same signing key. Keep a backup of this key.");
    let choices = [
        prompt::Choice {
            label: "generate",
            detail: "make a new release key",
        },
        prompt::Choice {
            label: "import",
            detail: "an existing JKS or PKCS12 file, or its base64 (password android)",
        },
    ];
    match prompt::choose(
        "Which release keystore should deploy sign with?",
        &choices,
        0,
    ) {
        Some(0) => generate_key(&saved, 4096)?,
        Some(_) => {
            let path = prompt::text("Keystore file:", "~/release.jks", |path| {
                let path = match path.strip_prefix("~/") {
                    Some(rest) => PathBuf::from(
                        std::env::var_os("HOME")
                            .or_else(|| std::env::var_os("USERPROFILE"))
                            .unwrap_or_default(),
                    )
                    .join(rest),
                    None => path.into(),
                };
                read_key(&path).map(|_| path.to_string_lossy().into_owned())
            })
            .ok_or("no keystore was given")?;
            save_key(&saved, &read_key(Path::new(&path))?)?;
        }
        None => return Err("no keystore was chosen".into()),
    }
    Ok(saved)
}

fn read_key(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.first() == Some(&0x30) || bytes.starts_with(&[0xfe, 0xed, 0xfe, 0xed]) {
        return Ok(bytes);
    }
    let encoded: Vec<u8> = bytes
        .into_iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| {
            format!(
                "{}: expected JKS, PKCS12 or base64 of one: {error}",
                path.display()
            )
        })
}

fn save_key(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(parent(path)).map_err(|error| error.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if path
        .extension()
        .is_some_and(|extension| extension == "base64")
    {
        file.write_all(
            base64::engine::general_purpose::STANDARD
                .encode(bytes)
                .as_bytes(),
        )
    } else {
        file.write_all(bytes)
    }
    .map_err(|error| error.to_string())
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub(crate) fn generate_key(path: &Path, bits: usize) -> Result<(), String> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    save_key(path, &new_keystore(bits)?)?;
    println!("Made Android signing key at {}", path.display());
    Ok(())
}

fn new_keystore(bits: usize) -> Result<Vec<u8>, String> {
    let key = RsaPrivateKey::new(&mut OsRng, bits).map_err(|error| error.to_string())?;
    let signer = rsa::pkcs1v15::SigningKey::<Sha256>::new(key.clone());
    let public = SubjectPublicKeyInfoOwned::from_key(key.to_public_key())
        .map_err(|error| error.to_string())?;
    let mut serial = [0; 16];
    OsRng.fill_bytes(&mut serial);
    serial[0] = serial[0] & 0x3f | 0x40;
    let certificate = CertificateBuilder::new(
        Profile::Root,
        SerialNumber::new(&serial).map_err(|error| error.to_string())?,
        Validity::from_now(Duration::from_secs(100 * 365 * 24 * 60 * 60))
            .map_err(|error| error.to_string())?,
        Name::from_str("CN=Block Android signing").map_err(|error| error.to_string())?,
        public,
        &signer,
    )
    .map_err(|error| error.to_string())?
    .build::<rsa::pkcs1v15::Signature>()
    .map_err(|error| error.to_string())?
    .to_der()
    .map_err(|error| error.to_string())?;
    let private = key.to_pkcs8_der().map_err(|error| error.to_string())?;
    let chain = p12_keystore::PrivateKeyChain::new(
        Sha256::digest(&certificate).to_vec(),
        p12_keystore::PrivateKey::from_der(private.as_bytes())
            .map_err(|error| error.to_string())?,
        [p12_keystore::Certificate::from_der(&certificate).map_err(|error| error.to_string())?],
    );
    let mut store = p12_keystore::KeyStore::new();
    store.add_entry(
        "android",
        p12_keystore::KeyStoreEntry::PrivateKeyChain(chain),
    );
    store
        .writer("android")
        .write()
        .map_err(|error| error.to_string())
}

struct SigningKey {
    key: RsaPrivateKey,
    certificate: Vec<u8>,
    public_key: Vec<u8>,
}

fn load_key(bytes: &[u8], password: &str) -> Result<SigningKey, String> {
    let (private, certificates) = if bytes.starts_with(&[0xfe, 0xed, 0xfe, 0xed]) {
        let mut store = jks::KeyStore::new();
        store
            .load(bytes, password.as_bytes())
            .map_err(|error| format!("JKS keystore: {error}"))?;
        let alias = store
            .aliases()
            .into_iter()
            .find(|alias| store.is_private_key_entry(alias))
            .ok_or("the keystore has no private key")?;
        let entry = store
            .get_private_key_entry(&alias, password.as_bytes())
            .map_err(|error| format!("JKS keystore: {error}"))?;
        let certificates = entry
            .certificate_chain
            .into_iter()
            .map(|certificate| certificate.content)
            .collect::<Vec<_>>();
        (entry.private_key, certificates)
    } else {
        let store = p12_keystore::KeyStore::from_pkcs12(
            bytes,
            password,
            p12_keystore::Pkcs12ImportPolicy::Strict,
        )
        .map_err(|error| format!("PKCS12 keystore: {error}"))?;
        let (_, chain) = store
            .private_key_chain()
            .ok_or("the keystore has no private key")?;
        let certificates = chain
            .certs()
            .iter()
            .map(|certificate| certificate.as_der().to_vec())
            .collect();
        (chain.key().as_der().to_vec(), certificates)
    };
    let key = RsaPrivateKey::from_pkcs8_der(&private)
        .map_err(|error| format!("the signing key must be RSA: {error}"))?;
    let public = key.to_public_key();
    for certificate in certificates {
        let public_key = x509_cert::Certificate::from_der(&certificate)
            .and_then(|parsed| parsed.tbs_certificate.subject_public_key_info.to_der())
            .map_err(|error| error.to_string())?;
        if RsaPublicKey::from_public_key_der(&public_key).is_ok_and(|key| key == public) {
            return Ok(SigningKey {
                key,
                certificate,
                public_key,
            });
        }
    }
    Err("the keystore has no certificate for its private key".into())
}

pub(crate) fn finalize(
    input: &Path,
    output: &Path,
    key: &Path,
    metadata: &Path,
) -> Result<(), String> {
    sign(input, output, key, Some(metadata), "android")
}

fn sign(
    input: &Path,
    output: &Path,
    key: &Path,
    metadata: Option<&Path>,
    password: &str,
) -> Result<(), String> {
    let key = load_key(&read_key(key)?, password)?;
    let mut apk = fs::read(input).map_err(|error| format!("{}: {error}", input.display()))?;
    if let Some(metadata) = metadata {
        let metadata =
            fs::read(metadata).map_err(|error| format!("{}: {error}", metadata.display()))?;
        apk = append_metadata(apk, &metadata)?;
    }
    let staged = output.with_extension("apk.partial");
    fs::create_dir_all(parent(output)).map_err(|error| error.to_string())?;
    let mut file = std::io::BufWriter::new(
        fs::File::create(&staged).map_err(|error| format!("{}: {error}", staged.display()))?,
    );
    sign_v2(&apk, &key, &mut file)?;
    file.flush()
        .map_err(|error| format!("{}: {error}", staged.display()))?;
    fs::rename(&staged, output).map_err(|error| format!("{}: {error}", output.display()))?;
    println!("Wrote {}", output.display());
    Ok(())
}

fn append_metadata(apk: Vec<u8>, metadata: &[u8]) -> Result<Vec<u8>, String> {
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    let mut apk = Cursor::new(apk);
    if ZipArchive::new(&mut apk)
        .map_err(|error| error.to_string())?
        .by_name("assets/build-info.json")
        .is_ok()
    {
        return Err(
            "APK already contains assets/build-info.json; use a template without build metadata"
                .into(),
        );
    }
    let mut archive = ZipWriter::new_append(apk).map_err(|error| error.to_string())?;
    archive
        .start_file(
            "assets/build-info.json",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|error| error.to_string())?;
    archive
        .write_all(metadata)
        .map_err(|error| error.to_string())?;
    Ok(archive
        .finish()
        .map_err(|error| error.to_string())?
        .into_inner())
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes([bytes[at], bytes[at + 1]]).into()
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn prefixed(parts: &[&[u8]]) -> Vec<u8> {
    let length: usize = parts.iter().map(|part| part.len()).sum();
    let mut out = (length as u32).to_le_bytes().to_vec();
    for part in parts {
        out.extend_from_slice(part);
    }
    out
}

fn sign_v2(apk: &[u8], key: &SigningKey, out: &mut impl Write) -> Result<(), String> {
    let end = apk.len().checked_sub(22).ok_or("the APK is not a zip")?;
    let eocd = (end.saturating_sub(0xffff)..=end)
        .rev()
        .find(|&at| apk[at..at + 4] == [0x50, 0x4b, 0x05, 0x06] && u16_at(apk, at + 20) == end - at)
        .ok_or("the APK has no end of central directory")?;
    let directory_size = u32_at(apk, eocd + 12);
    let directory = u32_at(apk, eocd + 16);
    if directory.checked_add(directory_size) != Some(eocd) {
        return Err("the APK's central directory does not end at its end record".into());
    }
    let mut entries_end = directory;
    if directory >= 32 && &apk[directory - 16..directory] == BLOCK_MAGIC {
        let size = u64::from_le_bytes(apk[directory - 24..directory - 16].try_into().unwrap());
        entries_end = usize::try_from(size)
            .ok()
            .and_then(|size| directory.checked_sub(size.checked_add(8)?))
            .ok_or("the APK's signing block is malformed")?;
    }

    let mut end_record = apk[eocd..].to_vec();
    end_record[16..20].copy_from_slice(&(entries_end as u32).to_le_bytes());
    let chunks: Vec<[u8; 32]> = [&apk[..entries_end], &apk[directory..eocd], &end_record]
        .into_iter()
        .flat_map(|section| section.chunks(CHUNK))
        .map(|chunk| {
            Sha256::new()
                .chain_update([0xa5])
                .chain_update((chunk.len() as u32).to_le_bytes())
                .chain_update(chunk)
                .finalize()
                .into()
        })
        .collect();
    let mut digest = Sha256::new()
        .chain_update([0x5a])
        .chain_update((chunks.len() as u32).to_le_bytes());
    for chunk in &chunks {
        digest.update(chunk);
    }
    let digest = digest.finalize();

    let algorithm = RSA_PKCS1_SHA256.to_le_bytes();
    let signed_data = [
        prefixed(&[&prefixed(&[&algorithm, &prefixed(&[&digest])])]),
        prefixed(&[&prefixed(&[&key.certificate])]),
        prefixed(&[]),
    ]
    .concat();
    let signature = key
        .key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &Sha256::digest(&signed_data))
        .map_err(|error| error.to_string())?;
    let signer = [
        prefixed(&[&signed_data]),
        prefixed(&[&prefixed(&[&algorithm, &prefixed(&[&signature])])]),
        prefixed(&[&key.public_key]),
    ]
    .concat();
    let value = prefixed(&[&prefixed(&[&signer])]);
    let pair = (4 + value.len() as u64).to_le_bytes();
    let size = (8 + 4 + value.len() as u64 + 8 + 16).to_le_bytes();
    let block = [
        &size[..],
        &pair,
        &V2_BLOCK.to_le_bytes(),
        &value,
        &size,
        BLOCK_MAGIC,
    ]
    .concat();

    let mut end_record = apk[eocd..].to_vec();
    end_record[16..20].copy_from_slice(&((entries_end + block.len()) as u32).to_le_bytes());
    for part in [
        &apk[..entries_end],
        &block,
        &apk[directory..eocd],
        &end_record,
    ] {
        out.write_all(part).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
