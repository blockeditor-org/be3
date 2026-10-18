use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Duration,
};

use sha2::{Digest, Sha256};

const USAGE: &str = "\
usage: ./scripts/deploy [--dry-run] [--allow-dirty] [--skip-server] [--skip-be] [--skip-web]
                        [--skip-downloads] [--skip-android] [--android-id ID]
                        [--android-label LABEL]

Builds be-server, the web bundle, be and the app for every platform in
release, uploads each be and each of the app's downloads to the mirror, swaps
the server on the VPS and publishes the web bundle through Bunny, points the mirror's download page at
the new build, and pins the uploaded be in scripts/internal/common.sh.

The downloads are the app with its plugins for Linux, macOS and Windows on
x86_64 and arm64 and the web bundle, each zipped, and the APK, signed with the
release keystore under the id and label --android-id and --android-label give
(com.be3.block and Block), so it installs beside CI's com.be3.block.ci. They go
to data/BUILD/app/ beside a page linking them, and once the server runs the build,
index.html is that page (https://be3.pfg.pw/). It is not
read back from the mirror, whose CDN may hold the last one until it is purged.

  --dry-run      print what it would do, uploading and changing nothing
  --allow-dirty  deploy a working tree that differs from its commit
  --skip-server  upload versioned artifacts without changing the server or live pages
  --skip-be      leave be as it is
  --skip-web     leave the web bundle as it is
  --skip-downloads  build and upload none of the app's downloads
  --skip-android    build and upload every download but the APK, which needs no keystore

./scripts/deploy asks at a terminal for any key it lacks and saves it in
~/.config/be3: the storage zone's password (be-app-storage-key), the API
key (bunny-api-key, required when publishing live pages) and the release keystore
(android-release-keystore.base64), which it can make anew.

Environment:
  BUNNY_STORAGE_KEY   the storage zone's password (be-app zone password for deployment artifacts)
  BUNNY_API_KEY       the account's API key, to purge mutable entry points from the CDN
  BE3_DEPLOY_STORAGE  where be and the downloads go (https://la.storage.bunnycdn.com/be-app)
  BE3_DEPLOY_MIRROR   where it is read back from (https://be3.pfg.pw)
  BE3_DEPLOY_HOST     the server, for ssh and scp (ubuntu@blocks.pfg.pw)
  BE3_DEPLOY_DIR      the directory there that holds be-server (be-server)
  BE3_DEPLOY_UNITS    the units stopped around the swap (be-server)
  Web files are uploaded under data/BUILD/web/; /web selects the live build.";

struct Options {
    repository: PathBuf,
    server: PathBuf,
    be: Vec<(String, PathBuf)>,
    dry_run: bool,
    allow_dirty: bool,
    skip_server: bool,
    skip_be: bool,
    web: Option<PathBuf>,
    skip_web: bool,
    downloads: Vec<(String, PathBuf)>,
    skip_downloads: bool,
}

pub fn main(arguments: Vec<String>) -> ExitCode {
    match parse(arguments.into_iter()).and_then(|options| run(&options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("deploy: {message}");
            ExitCode::FAILURE
        }
    }
}

fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        repository: PathBuf::new(),
        server: PathBuf::new(),
        be: Vec::new(),
        dry_run: false,
        allow_dirty: false,
        skip_server: false,
        skip_be: false,
        web: None,
        skip_web: false,
        downloads: Vec::new(),
        skip_downloads: false,
    };
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--repository" => options.repository = PathBuf::from(value()?),
            "--server" => options.server = PathBuf::from(value()?),
            "--be" => {
                let given = value()?;
                let (triple, path) = given
                    .split_once('=')
                    .ok_or_else(|| format!("--be takes TRIPLE=DIR, not {given}"))?;
                options.be.push((triple.to_owned(), PathBuf::from(path)));
            }
            "--dry-run" => options.dry_run = true,
            "--allow-dirty" => options.allow_dirty = true,
            "--skip-server" => options.skip_server = true,
            "--skip-be" => options.skip_be = true,
            "--web" => options.web = Some(PathBuf::from(value()?)),
            "--skip-web" => options.skip_web = true,
            "--download" => {
                let given = value()?;
                let (platform, path) = given
                    .split_once('=')
                    .ok_or_else(|| format!("--download takes PLATFORM=FILE, not {given}"))?;
                options
                    .downloads
                    .push((platform.to_owned(), PathBuf::from(path)));
            }
            "--skip-downloads" => options.skip_downloads = true,
            "--help" | "-h" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    if options.repository.as_os_str().is_empty() || options.server.as_os_str().is_empty() {
        return Err(format!("run this through ./scripts/deploy\n\n{USAGE}"));
    }
    Ok(options)
}

fn setting(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn run(options: &Options) -> Result<(), String> {
    let mut build = None;
    for (triple, bundle) in &options.be {
        let id = read_build(&bundle.join("build-id"))?;
        if build.as_ref().is_some_and(|build| *build != id) {
            return Err(format!("be for {triple} is build {id}, unlike the others"));
        }
        build = Some(id);
    }
    let build = build.ok_or("no be bundle was built")?;
    if !options.allow_dirty && build != head_tree(&options.repository)? {
        return Err(format!(
            "this is build {build}: the working tree differs from its commit, so nobody could \
             build it again. Commit first, or pass --allow-dirty"
        ));
    }
    println!("deploying build {build}");

    let storage = setting(
        "BE3_DEPLOY_STORAGE",
        "https://la.storage.bunnycdn.com/be-app",
    );
    let mirror = setting("BE3_DEPLOY_MIRROR", "https://be3.pfg.pw");
    let key = std::env::var("BUNNY_STORAGE_KEY").unwrap_or_default();
    let key = key.trim();
    if key.is_empty()
        && !options.dry_run
        && (!options.skip_be
            || !options.skip_downloads
            || !options.skip_web
            || !options.skip_server)
    {
        return Err("set BUNNY_STORAGE_KEY to the storage zone's password".to_owned());
    }
    let web = options.web.as_ref().filter(|_| !options.skip_web);
    let publishes_pages = !options.skip_server
        && (web.is_some() || (!options.skip_downloads && !options.downloads.is_empty()));
    if publishes_pages
        && !options.dry_run
        && std::env::var("BUNNY_API_KEY")
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err("set BUNNY_API_KEY to purge the CDN before publishing live pages".to_owned());
    }

    if let Some(web) = web {
        let id = read_build(&web.join("build-id"))?;
        if id != build {
            return Err(format!("the web bundle is build {id}, not {build}"));
        }
    }
    let web_entry = web
        .map(|web| {
            let html = std::fs::read_to_string(web.join("index.html"))
                .map_err(|error| format!("cannot read web index: {error}"))?;
            web_entry_point(&html, &build)
        })
        .transpose()?;
    let agents = Agents::new();
    let store = Store {
        options,
        agents: &agents,
        storage: &storage,
        mirror: &mirror,
        key,
    };

    let mut pins = Vec::new();
    if !options.skip_be {
        for (triple, bundle) in &options.be {
            let directory = format!("data/{build}/be-{triple}");
            for name in ["be", "build-id"] {
                let bytes = std::fs::read(bundle.join(name)).map_err(|error| {
                    format!("cannot read {}: {error}", bundle.join(name).display())
                })?;
                let sha256 = store.put(
                    &bundle.join(name).display().to_string(),
                    &format!("{directory}/{name}"),
                    &bytes,
                    "application/octet-stream",
                    true,
                )?;
                if name == "be" {
                    pins.push((triple.clone(), sha256));
                }
            }
        }
    }

    let mut page = None;
    if !options.skip_downloads && !options.downloads.is_empty() {
        let mut listed = Vec::new();
        for (platform, file) in &options.downloads {
            let extension = file.extension().and_then(|extension| extension.to_str());
            let name = format!("block-app-{platform}.{}", extension.unwrap_or("zip"));
            let bytes = std::fs::read(file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
            let sha256 = store.put(
                &file.display().to_string(),
                &format!("data/{build}/app/{name}"),
                &bytes,
                "application/octet-stream",
                true,
            )?;
            listed.push(Download {
                label: label(platform),
                name,
                size: bytes.len(),
                sha256,
            });
        }
        store.put(
            "the download page",
            &format!("data/{build}/app/index.html"),
            download_page(&build, &listed, "").as_bytes(),
            "text/html; charset=utf-8",
            true,
        )?;
        page = Some(download_page(
            &build,
            &listed,
            &format!("/data/{build}/app/"),
        ));
    }

    if let Some(web) = web {
        upload_web(&store, web, web, &format!("data/{build}/web"))?;
    }
    if !options.skip_server {
        let bytes = std::fs::read(&options.server)
            .map_err(|error| format!("cannot read server: {error}"))?;
        store.put(
            "be-server",
            &format!("data/{build}/server/be-server"),
            &bytes,
            "application/octet-stream",
            true,
        )?;
        store.put(
            "server build-id",
            &format!("data/{build}/server/build-id"),
            build.as_bytes(),
            "text/plain; charset=utf-8",
            true,
        )?;
    }

    if !options.skip_server {
        let host = setting("BE3_DEPLOY_HOST", "ubuntu@blocks.pfg.pw");
        let directory = setting("BE3_DEPLOY_DIR", "be-server");
        let units = setting("BE3_DEPLOY_UNITS", "be-server");
        let build_id = options
            .be
            .first()
            .map(|(_, bundle)| bundle.join("build-id"))
            .ok_or("no build-id to send")?;
        let server = options.server.display().to_string();
        let build_id = build_id.display().to_string();
        shell(
            options,
            "scp",
            &[&server, &format!("{host}:{directory}/be-server.new")],
        )?;
        shell(
            options,
            "scp",
            &[&build_id, &format!("{host}:{directory}/build-id.new")],
        )?;
        shell(
            options,
            "ssh",
            &[&host, &format!("sudo systemctl stop {units}")],
        )?;
        let moved = shell(
            options,
            "ssh",
            &[
                &host,
                &format!(
                    "chmod +x {directory}/be-server.new && mv {directory}/be-server.new {directory}/be-server && mv {directory}/build-id.new {directory}/build-id"
                ),
            ],
        );
        let started = shell(
            options,
            "ssh",
            &[&host, &format!("sudo systemctl start {units}")],
        );
        moved?;
        started?;
    }

    if let Some(launch) = web_entry.filter(|_| !options.skip_server) {
        store.put(
            "the web app entry point",
            "web",
            launch.as_bytes(),
            "text/html; charset=utf-8",
            false,
        )?;
        purge_pages(options, &agents, &mirror, &["web"])?;
    }

    if page.is_some() && options.skip_server {
        println!(
            "the server still runs its last build, so {mirror}/ still offers that one's \
             downloads; this build's are at {mirror}/data/{build}/app/"
        );
    } else if let Some(page) = page {
        store.put(
            "the latest download page",
            "index.html",
            page.as_bytes(),
            "text/html; charset=utf-8",
            false,
        )?;
        purge_pages(options, &agents, &mirror, &["index.html", ""])?;
        println!("the downloads for build {build} are at {mirror}/");
    }

    if !pins.is_empty() {
        let common = options.repository.join("scripts/internal/common.sh");
        if options.dry_run {
            println!("pin be {build} from {mirror} in {}", common.display());
        } else {
            pin(&common, &build, &pins, &mirror)?;
            println!(
                "pinned be {build} in {}; commit it so ./scripts/be runs this build",
                common.display()
            );
        }
    }
    Ok(())
}

struct Store<'a> {
    options: &'a Options,
    agents: &'a Agents,
    storage: &'a str,
    mirror: &'a str,
    key: &'a str,
}

impl Store<'_> {
    fn put(
        &self,
        what: &str,
        path: &str,
        bytes: &[u8],
        content_type: &str,
        read_back: bool,
    ) -> Result<String, String> {
        let sha256 = hex(&Sha256::digest(bytes));
        let target = format!("{}/{path}", self.storage);
        println!(
            "upload {what} ({} bytes, sha256 {sha256}) to {target}",
            bytes.len()
        );
        if self.options.dry_run {
            return Ok(sha256);
        }
        if read_back {
            match stored_sha256(self.agents.to(&target), &target, self.key)? {
                Some(existing) if existing != sha256 => {
                    return Err(format!(
                        "refusing to overwrite immutable artifact {target} with different bytes; build a new release"
                    ));
                }
                Some(_) => println!("  identical immutable artifact already exists"),
                None => upload(
                    self.agents.to(&target),
                    &target,
                    self.key,
                    bytes,
                    &sha256,
                    content_type,
                )?,
            }
        } else {
            upload(
                self.agents.to(&target),
                &target,
                self.key,
                bytes,
                &sha256,
                content_type,
            )?;
        }
        if !read_back {
            return Ok(sha256);
        }
        let source = format!("{}/{path}", self.mirror);
        let read = download(
            self.agents.to(&source),
            &source,
            path.starts_with("data/") && path.ends_with("/web/index.html"),
        )?;
        if hex(&Sha256::digest(&read)) != sha256 {
            return Err(format!(
                "{source} is not what was uploaded to {target}; check \
                 BE3_DEPLOY_STORAGE and BE3_DEPLOY_MIRROR name the same place"
            ));
        }
        println!("  read back from {source}");
        Ok(sha256)
    }
}

struct Download {
    label: &'static str,
    name: String,
    size: usize,
    sha256: String,
}

fn label(platform: &str) -> &'static str {
    match platform {
        "linux-x86_64" => "Linux (x86_64)",
        "linux-arm64" => "Linux (arm64)",
        "macos-arm64" => "macOS (Apple silicon)",
        "macos-x86_64" => "macOS (Intel)",
        "windows-x86_64" => "Windows (x86_64)",
        "windows-arm64" => "Windows (arm64)",
        "android" => "Android",
        "web" => "Web (to serve yourself)",
        _ => "Other",
    }
}

fn download_page(build: &str, downloads: &[Download], prefix: &str) -> String {
    let rows: String = downloads
        .iter()
        .map(|download| {
            format!(
                "<tr><td>{}</td><td><a href=\"{prefix}{name}\">{name}</a></td><td>{:.1} MB</td><td><code>{}</code></td></tr>\n",
                download.label,
                download.size as f64 / 1_000_000.0,
                download.sha256,
                name = download.name,
            )
        })
        .collect();
    format!(
        "<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>Block downloads</title>
<style>
:root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
body {{ max-width: 60rem; margin: 2rem auto; padding: 0 1rem; }}
table {{ border-collapse: collapse; width: 100%; }}
td, th {{ text-align: left; padding: 0.4rem 0.8rem 0.4rem 0; vertical-align: top; }}
code {{ font-size: 0.75rem; word-break: break-all; }}
</style>
</head>
<body>
<h1>Block</h1>
<p>Build <code>{build}</code>. A download connects only to a server running the same build.</p>
<table>
<tr><th>Platform</th><th>Download</th><th>Size</th><th>SHA-256</th></tr>
{rows}</table>
<p>Each zip holds the app in a folder of its own: run <code>block-app</code> (<code>block-app.exe</code> on Windows) inside it. The macOS builds are not signed, so macOS asks before opening them; <code>xattr -dr com.apple.quarantine block-app-macos-*</code> lets them open. The web bundle is also available to serve yourself. <a href=\"https://be3.pfg.pw/web\">Open the hosted web app</a>.</p>
</body>
</html>
"
    )
}

fn purge(agents: &Agents, api_key: &str, url: &str) -> Result<(), String> {
    let api = "https://api.bunny.net/purge";
    agents
        .to(api)
        .post(api)
        .query("url", url)
        .set("AccessKey", api_key)
        .call()
        .map_err(|error| format!("purging {url} from the CDN failed: {error}"))?;
    println!("purged {url}");
    Ok(())
}

struct Agents {
    proxied: ureq::Agent,
    direct: ureq::Agent,
}

impl Agents {
    fn new() -> Self {
        let builder = || ureq::AgentBuilder::new().timeout(Duration::from_secs(600));
        Self {
            proxied: builder().try_proxy_from_env(true).build(),
            direct: builder().build(),
        }
    }

    fn to(&self, url: &str) -> &ureq::Agent {
        match url.starts_with("https://") {
            true => &self.proxied,
            false => &self.direct,
        }
    }
}

fn web_entry_point(html: &str, build: &str) -> Result<String, String> {
    let (before, after) = html
        .split_once("<head>")
        .ok_or("the web entry point has no head element")?;
    Ok(format!(
        "{before}<head><base href=\"/data/{build}/web/\">{after}"
    ))
}

fn upload_web(
    store: &Store<'_>,
    root: &Path,
    directory: &Path,
    prefix: &str,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
    paths.sort();
    for path in paths {
        if path.is_dir() {
            upload_web(store, root, &path, prefix)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_str()
                .ok_or("a web filename is not UTF-8")?
                .replace('\\', "/");
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            store.put(
                &path.display().to_string(),
                &format!("{prefix}/{relative}"),
                &bytes,
                web_content_type(&path),
                true,
            )?;
        }
    }
    Ok(())
}

fn web_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn purge_pages(
    options: &Options,
    agents: &Agents,
    mirror: &str,
    paths: &[&str],
) -> Result<(), String> {
    let key = std::env::var("BUNNY_API_KEY").unwrap_or_default();
    for path in paths {
        let url = format!("{mirror}/{path}");
        if options.dry_run {
            println!("purge {url}");
        } else {
            purge(agents, key.trim(), &url)?;
        }
    }
    Ok(())
}

fn head_tree(repository: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "HEAD^{tree}"])
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git cannot name the tree of HEAD: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn read_build(path: &Path) -> Result<String, String> {
    let id = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let id = id.trim();
    if id.is_empty() || id == "unknown" {
        return Err(format!("{} names no build", path.display()));
    }
    Ok(id.to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stored_sha256(agent: &ureq::Agent, url: &str, key: &str) -> Result<Option<String>, String> {
    let response = match agent.get(url).set("AccessKey", key).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(error) => return Err(format!("checking immutable artifact {url} failed: {error}")),
    };
    let mut reader = response.into_reader();
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("reading immutable artifact {url} failed: {error}"))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(Some(hex(&hash.finalize())))
}

fn upload(
    agent: &ureq::Agent,
    url: &str,
    key: &str,
    bytes: &[u8],
    sha256: &str,
    content_type: &str,
) -> Result<(), String> {
    agent
        .put(url)
        .set("AccessKey", key)
        .set("Checksum", &sha256.to_ascii_uppercase())
        .set("Content-Type", content_type)
        .send_bytes(bytes)
        .map_err(|error| format!("uploading to {url} failed: {error}"))?;
    Ok(())
}

fn download(agent: &ureq::Agent, url: &str, web_entry: bool) -> Result<Vec<u8>, String> {
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("reading {url} back failed: {error}"))?;
    if web_entry {
        for (name, expected) in [
            ("Cross-Origin-Opener-Policy", "same-origin"),
            ("Cross-Origin-Embedder-Policy", "require-corp"),
        ] {
            if response.header(name) != Some(expected) {
                return Err(format!(
                    "{url} needs {name}: {expected}; configure the Bunny pull-zone response headers before deployment"
                ));
            }
        }
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("reading {url} back failed: {error}"))?;
    Ok(bytes)
}

fn shell(options: &Options, program: &str, arguments: &[&str]) -> Result<(), String> {
    println!("{program} {}", arguments.join(" "));
    if options.dry_run {
        return Ok(());
    }
    let status = Command::new(program)
        .args(arguments)
        .status()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    match status.success() {
        true => Ok(()),
        false => Err(format!(
            "{program} {} failed ({status})",
            arguments.join(" ")
        )),
    }
}

fn pin(common: &Path, build: &str, pins: &[(String, String)], mirror: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(common)
        .map_err(|error| format!("cannot read {}: {error}", common.display()))?;
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut set = |name: &str, value: &str| -> Result<(), String> {
        let line = lines
            .iter_mut()
            .find(|line| line.starts_with(&format!("{name}=")))
            .ok_or_else(|| format!("{} has no {name}= line", common.display()))?;
        *line = format!("{name}='{value}'");
        Ok(())
    };
    set("be_release", build)?;
    set("be_mirror", &format!("{mirror}/data"))?;
    for (triple, sha256) in pins {
        set(&format!("be_sha256_{}", triple.replace('-', "_")), sha256)?;
    }
    let mut written = lines.join("\n");
    if text.ends_with('\n') {
        written.push('\n');
    }
    std::fs::write(common, written)
        .map_err(|error| format!("cannot write {}: {error}", common.display()))
}

#[cfg(test)]
mod tests;
