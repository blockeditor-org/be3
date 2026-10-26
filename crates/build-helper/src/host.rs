use std::{
    fs,
    io::{self, Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

use sha2::{Digest as _, Sha256};

use crate::{
    products::{Manifest, ManifestFile, Product, manifest_url},
    queue::{Backend, Log},
};

pub const STORAGE: &str = "https://storage.bunnycdn.com/be3-ci";

const KEPT: [&str; 5] = [
    "buck-out",
    "target",
    ".build-server-key.*",
    ".gateway-authorization",
    ".namespace-token.json",
];

pub struct Host {
    pub checkout: PathBuf,
    pub state: PathBuf,
    pub keystore: Option<PathBuf>,
    pub public: String,
    pub storage_key: Option<String>,
    pub max_buck_out: u64,
    pub git: Mutex<()>,
}

struct Built {
    files: Vec<(String, PathBuf)>,
    executable: String,
}

impl Backend for Host {
    fn published(&self, product: &str, commit: &str) -> bool {
        let marker = self.published_marker(product, commit);
        if marker.exists() {
            return true;
        }
        let found = match &self.storage_key {
            Some(key) => curl_status(
                &["--request", "GET"],
                Some(key),
                &format!("{STORAGE}/builds/{product}/{commit}.json"),
            ),
            None => curl_status(
                &["--head"],
                None,
                &manifest_url(&self.public, product, commit),
            ),
        } == Some(200);
        if found {
            let _ = mark(&marker);
        }
        found
    }

    fn allowed(&self, commit: &str) -> Result<(), String> {
        let _git = self.git.lock().unwrap();
        let fetch = Command::new("git")
            .current_dir(&self.checkout)
            .args([
                "fetch",
                "--quiet",
                "--prune",
                "origin",
                "+refs/heads/*:refs/remotes/origin/*",
            ])
            .output()
            .map_err(|error| format!("could not run git: {error}"))?;
        if !fetch.status.success() {
            tracing::warn!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&fetch.stderr).trim()
            );
        }
        let contains = Command::new("git")
            .current_dir(&self.checkout)
            .args(["branch", "-r", "--contains", commit])
            .stderr(Stdio::null())
            .output()
            .map_err(|error| format!("could not run git: {error}"))?;
        if contains.status.success() && !String::from_utf8_lossy(&contains.stdout).trim().is_empty()
        {
            Ok(())
        } else {
            Err(format!(
                "{commit} is not on a branch of the repository; only commits on its branches are built"
            ))
        }
    }

    fn build(&self, product: Product, commit: &str, log: &Log) -> Result<(), String> {
        let name = product.name();
        log.line(&format!("Building {name} at {commit}"));
        {
            let _git = self.git.lock().unwrap();
            run(
                log,
                Command::new("git")
                    .current_dir(&self.checkout)
                    .args(["checkout", "--detach", "--force", commit]),
            )?;
            let mut clean = Command::new("git");
            clean.current_dir(&self.checkout).args(["clean", "-ffdx"]);
            for kept in KEPT {
                clean.args(["-e", &format!("/{kept}")]);
            }
            run(log, &mut clean)?;
        }
        let built = self.build_product(product, log);
        self.clean_if_large(log);
        let built = built?;
        let mut files = Vec::new();
        for (path, file) in &built.files {
            let (sha256, size) =
                hash(file).map_err(|error| format!("{}: {error}", file.display()))?;
            let mode = fs::metadata(file)
                .map_err(|error| format!("{}: {error}", file.display()))?
                .permissions()
                .mode();
            self.upload_file(log, file, &sha256)?;
            files.push(ManifestFile {
                path: path.clone(),
                sha256,
                size,
                executable: *path == built.executable || mode & 0o111 != 0,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = Manifest {
            product: name.clone(),
            commit: commit.to_owned(),
            files,
        };
        let json = serde_json::to_vec_pretty(&manifest).unwrap();
        let saved = self
            .state
            .join("manifests")
            .join(&name)
            .join(format!("{commit}.json"));
        fs::create_dir_all(saved.parent().unwrap()).map_err(|error| error.to_string())?;
        fs::write(&saved, &json).map_err(|error| format!("{}: {error}", saved.display()))?;
        let Some(key) = &self.storage_key else {
            log.line(&format!(
                "No BUNNY_STORAGE_KEY, so nothing was uploaded; the manifest is at {}",
                saved.display()
            ));
            return Ok(());
        };
        put(
            key,
            &saved,
            "application/json",
            &format!("{STORAGE}/builds/{name}/{commit}.json"),
        )?;
        mark(&self.published_marker(&name, commit)).map_err(|error| error.to_string())?;
        log.line(&format!(
            "Published {}",
            manifest_url(&self.public, &name, commit)
        ));
        Ok(())
    }
}

impl Host {
    fn published_marker(&self, product: &str, commit: &str) -> PathBuf {
        self.state.join("published").join(product).join(commit)
    }

    fn buck(&self, log: &Log, arguments: &[&str]) -> Result<Vec<(String, PathBuf)>, String> {
        let mut command = Command::new(self.checkout.join("scripts/buck"));
        command
            .current_dir(&self.checkout)
            .args(["build", "--show-full-output"])
            .args(arguments);
        let stdout = run(log, &mut command)?;
        Ok(String::from_utf8_lossy(&stdout)
            .lines()
            .filter_map(|line| {
                let (target, path) = line.split_once(' ')?;
                let target = target.strip_prefix("root").unwrap_or(target);
                target
                    .starts_with("//")
                    .then(|| (target.to_owned(), self.checkout.join(path.trim())))
            })
            .collect())
    }

    fn build_product(&self, product: Product, log: &Log) -> Result<Built, String> {
        let output = |outputs: &[(String, PathBuf)], target: &str| {
            outputs
                .iter()
                .find(|(built, _)| built == target)
                .map(|(_, path)| path.clone())
                .ok_or_else(|| format!("buck printed no output for {target}"))
        };
        match product {
            Product::Android | Product::LauncherAndroid => {
                let (target, apk) = if product == Product::Android {
                    ("//crates/block-app:android-dist", "block-app.apk")
                } else {
                    ("//crates/be-launcher:android-dist", "be-launcher.apk")
                };
                let keystore = self.keystore.as_ref().ok_or(
                    "the build helper was started without --keystore, so it cannot sign APKs",
                )?;
                let outputs = self.buck(
                    log,
                    &[
                        target,
                        "//buck/app:build-info.json",
                        "//crates/local-tools:release",
                    ],
                )?;
                let template = output(&outputs, target)?;
                let info = output(&outputs, "//buck/app:build-info.json")?;
                let tool = output(&outputs, "//crates/local-tools:release")?;
                let signed = self.state.join("work").join(apk);
                let _ = fs::remove_file(&signed);
                run(
                    log,
                    Command::new(tool)
                        .current_dir(&self.checkout)
                        .arg("apk-sign")
                        .arg("--in")
                        .arg(template)
                        .arg("--out")
                        .arg(&signed)
                        .arg("--keystore")
                        .arg(keystore)
                        .arg("--build-info")
                        .arg(info),
                )?;
                Ok(Built {
                    files: vec![(apk.to_owned(), signed)],
                    executable: String::new(),
                })
            }
            Product::Native(platform) | Product::BeuiDemo(platform) => {
                let target = match product {
                    Product::Native(_) => "//crates/block-app:native",
                    _ => "//crates/beui-demo:native",
                };
                let platforms = format!("root//buck/platforms:{platform}");
                let outputs = self.buck(
                    log,
                    &["-m", "release", "--target-platforms", &platforms, target],
                )?;
                let path = output(&outputs, target)?;
                let windows = platform.starts_with("windows");
                let mut files = Vec::new();
                if path.is_dir() {
                    collect(&path, "", &mut files).map_err(|error| error.to_string())?;
                    let executable = if windows {
                        "block-app.exe"
                    } else {
                        "block-app"
                    };
                    if !files.iter().any(|(name, _)| name == executable) {
                        return Err(format!("{target} staged no {executable}"));
                    }
                    Ok(Built {
                        files,
                        executable: executable.to_owned(),
                    })
                } else {
                    let executable = if windows {
                        "beui-demo.exe"
                    } else {
                        "beui-demo"
                    };
                    files.push((executable.to_owned(), path));
                    Ok(Built {
                        files,
                        executable: executable.to_owned(),
                    })
                }
            }
        }
    }

    fn upload_file(&self, log: &Log, file: &Path, sha256: &str) -> Result<(), String> {
        let marker = self.state.join("uploaded").join(sha256);
        if marker.exists() {
            return Ok(());
        }
        let public = format!("{}/files/{sha256}", self.public.trim_end_matches('/'));
        if curl_status(&["--head"], None, &public) == Some(200) {
            let _ = mark(&marker);
            return Ok(());
        }
        let Some(key) = &self.storage_key else {
            log.line(&format!(
                "Would upload {} as files/{sha256}",
                file.display()
            ));
            return Ok(());
        };
        put(
            key,
            file,
            "application/octet-stream",
            &format!("{STORAGE}/files/{sha256}"),
        )?;
        log.line(&format!("Uploaded files/{sha256}"));
        mark(&marker).map_err(|error| error.to_string())
    }

    fn clean_if_large(&self, log: &Log) {
        let size = size(&self.checkout.join("buck-out"));
        if size <= self.max_buck_out {
            return;
        }
        log.line(&format!(
            "buck-out holds {} GB, more than {} GB; cleaning it",
            size >> 30,
            self.max_buck_out >> 30
        ));
        let _ = run(
            log,
            Command::new(self.checkout.join("scripts/buck"))
                .current_dir(&self.checkout)
                .arg("clean"),
        );
    }
}

fn run(log: &Log, command: &mut Command) -> Result<Vec<u8>, String> {
    let shown = format!("{command:?}");
    let (mut reader, writer) = io::pipe().map_err(|error| error.to_string())?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(writer)
        .spawn()
        .map_err(|error| format!("could not run {shown}: {error}"))?;
    command.stderr(Stdio::null());
    let mut stdout = child.stdout.take().unwrap();
    let collected = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut chunk = [0u8; 8192];
            while let Ok(read) = reader.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                log.write(&chunk[..read]);
            }
        });
        let mut collected = Vec::new();
        let mut chunk = [0u8; 8192];
        while let Ok(read) = stdout.read(&mut chunk) {
            if read == 0 {
                break;
            }
            log.write(&chunk[..read]);
            collected.extend_from_slice(&chunk[..read]);
        }
        collected
    });
    let status = child.wait().map_err(|error| format!("{shown}: {error}"))?;
    if !status.success() {
        return Err(format!("{shown} failed ({status})"));
    }
    Ok(collected)
}

fn collect(directory: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| io::Error::other(format!("{}: not UTF-8", entry.path().display())))?;
        let relative = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        let path = entry.path();
        if fs::metadata(&path)?.is_dir() {
            collect(&path, &relative, files)?;
        } else {
            files.push((relative, path));
        }
    }
    Ok(())
}

fn size(directory: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(metadata) if metadata.is_dir() => size(&entry.path()),
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        })
        .sum()
}

fn hash(path: &Path) -> io::Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 1 << 20];
    let mut size = 0;
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&chunk[..read]);
    }
    let digest = hasher.finalize();
    Ok((
        digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        size,
    ))
}

fn mark(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, "")
}

fn curl(arguments: &[&str], key: Option<&str>, url: &str) -> io::Result<std::process::Output> {
    let mut child = Command::new("curl")
        .args(["--silent", "--show-error", "--retry", "3", "--config", "-"])
        .args(arguments)
        .arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    if let Some(key) = key {
        stdin.write_all(format!("header = \"AccessKey: {key}\"\n").as_bytes())?;
    }
    drop(stdin);
    child.wait_with_output()
}

fn curl_status(arguments: &[&str], key: Option<&str>, url: &str) -> Option<u16> {
    let mut all = vec!["--output", "/dev/null", "--write-out", "%{http_code}"];
    all.extend_from_slice(arguments);
    let output = curl(&all, key, url).ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn put(key: &str, file: &Path, content_type: &str, url: &str) -> Result<(), String> {
    let file = file.to_str().ok_or("a path that is not UTF-8")?;
    let content_type = format!("Content-Type: {content_type}");
    let output = curl(
        &[
            "--fail",
            "--output",
            "/dev/null",
            "--request",
            "PUT",
            "--header",
            &content_type,
            "--upload-file",
            file,
        ],
        Some(key),
        url,
    )
    .map_err(|error| format!("could not run curl: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "uploading to {url} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
