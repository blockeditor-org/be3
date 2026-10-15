use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Duration,
};

use sha2::{Digest, Sha256};

const USAGE: &str = "\
usage: ./scripts/deploy [--dry-run] [--allow-dirty] [--skip-server] [--skip-be] [--skip-web]
                        [--skip-downloads] [--android-id ID] [--android-label LABEL]

Builds be-server, the web bundle, be and the app for every platform in
release, uploads each be and each of the app's downloads to the mirror, swaps
the server and the web bundle on the VPS, points the mirror's download page at
the new build, and pins the uploaded be in scripts/internal/common.sh.

The downloads are the app with its plugins for Linux, macOS and Windows on
x86_64 and arm64 and the web bundle, each zipped, and the APK, signed with the
release keystore under the id and label --android-id and --android-label give
(com.be3.block and Block), so it installs beside CI's com.be3.block.ci. They go
to app/BUILD/ beside a page linking them, and app/index.html is that page for
the build the server runs (https://lfs.pfg.pw/by-name/be3/app/).

  --dry-run      print what it would do, uploading and changing nothing
  --allow-dirty  deploy a working tree that differs from its commit
  --skip-server  only upload be and pin it
  --skip-be      leave be as it is
  --skip-web     leave the web bundle as it is
  --skip-downloads  upload none of the app's downloads

./scripts/deploy asks at a terminal for any key it lacks and saves it in
~/.config/be3: the storage zone's password (bunny-storage-key), the API
key (bunny-api-key, which may be empty) and the release keystore
(android-release-keystore.base64), which it can make anew.

Environment:
  BUNNY_STORAGE_KEY   the storage zone's password (needed to upload be and the downloads)
  BUNNY_API_KEY       the account's API key, to purge the download page from the CDN
  BE3_DEPLOY_STORAGE  where be and the downloads go (https://la.storage.bunnycdn.com/clfs/by-name/be3)
  BE3_DEPLOY_MIRROR   where it is read back from (https://lfs.pfg.pw/by-name/be3)
  BE3_DEPLOY_HOST     the server, for ssh and scp (ubuntu@blocks.pfg.pw)
  BE3_DEPLOY_DIR      the directory there that holds be-server (be-server)
  BE3_DEPLOY_UNITS    the units stopped around the swap (caddy be-server)
  BE3_DEPLOY_WEB      where Caddy serves the web bundle from (/srv/be3-web)";

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
        "https://la.storage.bunnycdn.com/clfs/by-name/be3",
    );
    let mirror = setting("BE3_DEPLOY_MIRROR", "https://lfs.pfg.pw/by-name/be3");
    let key = std::env::var("BUNNY_STORAGE_KEY").unwrap_or_default();
    let key = key.trim();
    if key.is_empty() && !options.dry_run && !(options.skip_be && options.skip_downloads) {
        return Err("set BUNNY_STORAGE_KEY to the storage zone's password".to_owned());
    }
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
            let directory = format!("be/{build}/be-{triple}");
            for name in ["be", "build-id"] {
                let bytes = std::fs::read(bundle.join(name)).map_err(|error| {
                    format!("cannot read {}: {error}", bundle.join(name).display())
                })?;
                let sha256 = store.put(
                    &bundle.join(name).display().to_string(),
                    &format!("{directory}/{name}"),
                    &bytes,
                    "application/octet-stream",
                )?;
                if name == "be" {
                    pins.push((triple.clone(), sha256));
                }
            }
        }
    }

    let mut page = None;
    if !options.skip_downloads {
        let mut listed = Vec::new();
        for (platform, file) in &options.downloads {
            let extension = file.extension().and_then(|extension| extension.to_str());
            let name = format!("block-app-{platform}.{}", extension.unwrap_or("zip"));
            let bytes = std::fs::read(file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
            let sha256 = store.put(
                &file.display().to_string(),
                &format!("app/{build}/{name}"),
                &bytes,
                "application/octet-stream",
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
            &format!("app/{build}/index.html"),
            download_page(&build, &listed, "").as_bytes(),
            "text/html; charset=utf-8",
        )?;
        page = Some(download_page(&build, &listed, &format!("{build}/")));
    }

    if !options.skip_server {
        let host = setting("BE3_DEPLOY_HOST", "ubuntu@blocks.pfg.pw");
        let directory = setting("BE3_DEPLOY_DIR", "be-server");
        let units = setting("BE3_DEPLOY_UNITS", "caddy be-server");
        let build_id = options
            .be
            .first()
            .map(|(_, bundle)| bundle.join("build-id"))
            .ok_or("no build-id to send")?;
        let server = options.server.display().to_string();
        let build_id = build_id.display().to_string();
        let web = options.web.as_ref().filter(|_| !options.skip_web);
        let served = setting("BE3_DEPLOY_WEB", "/srv/be3-web");
        if let Some(web) = web {
            let id = read_build(&web.join("build-id"))?;
            if id != build {
                return Err(format!("the web bundle is build {id}, not {build}"));
            }
            send_tree(
                options,
                web,
                &host,
                &format!(
                    "rm -rf {directory}/web.new && mkdir -p {directory}/web.new && tar -xzf - -C {directory}/web.new"
                ),
            )?;
        }
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
                &[
                    format!(
                        "chmod +x {directory}/be-server.new && mv {directory}/be-server.new {directory}/be-server && mv {directory}/build-id.new {directory}/build-id"
                    ),
                    web.map(|_| {
                        format!(
                            " && sudo rm -rf {served}.old && {{ [ ! -e {served} ] || sudo mv {served} {served}.old; }} && sudo mv {directory}/web.new {served} && sudo chmod -R a+rX {served}"
                        )
                    })
                    .unwrap_or_default(),
                ]
                .concat(),
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

    if let Some(page) = page {
        store.put(
            "the latest download page",
            "app/index.html",
            page.as_bytes(),
            "text/html; charset=utf-8",
        )?;
        let url = format!("{mirror}/app/index.html");
        let api_key = std::env::var("BUNNY_API_KEY").unwrap_or_default();
        if options.dry_run {
            println!("purge {url} and {mirror}/app/");
        } else if api_key.trim().is_empty() {
            println!(
                "BUNNY_API_KEY is not set, so the CDN may serve the last download page at \
                 {mirror}/app/ until its copy expires"
            );
        } else {
            for url in [url, format!("{mirror}/app/")] {
                purge(&agents, api_key.trim(), &url)?;
            }
        }
        println!("the downloads for build {build} are at {mirror}/app/");
    }

    if !pins.is_empty() {
        let common = options.repository.join("scripts/internal/common.sh");
        if options.dry_run {
            println!("pin be {build} in {}", common.display());
        } else {
            pin(&common, &build, &pins)?;
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
        upload(
            self.agents.to(&target),
            &target,
            self.key,
            bytes,
            &sha256,
            content_type,
        )?;
        let source = format!("{}/{path}", self.mirror);
        let read = download(self.agents.to(&source), &source)?;
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
<p>Each zip holds the app in a folder of its own: run <code>block-app</code> (<code>block-app.exe</code> on Windows) inside it. The macOS builds are not signed, so macOS asks before opening them; <code>xattr -dr com.apple.quarantine block-app-macos-*</code> lets them open. The web bundle is what <code>./scripts/deploy</code> serves, to put behind a server of your own.</p>
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

fn send_tree(options: &Options, tree: &Path, host: &str, unpack: &str) -> Result<(), String> {
    println!("tar -czf - -C {} . | ssh {host} {unpack}", tree.display());
    if options.dry_run {
        return Ok(());
    }
    let mut tar = Command::new("tar")
        .args(["-czf", "-", "-C"])
        .arg(tree)
        .arg(".")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run tar: {error}"))?;
    let packed = tar.stdout.take().ok_or("tar gave no output")?;
    let unpacked = Command::new("ssh")
        .args([host, unpack])
        .stdin(packed)
        .status()
        .map_err(|error| format!("could not run ssh: {error}"))?;
    let packed = tar.wait().map_err(|error| format!("tar failed: {error}"))?;
    match (packed.success(), unpacked.success()) {
        (true, true) => Ok(()),
        _ => Err(format!("sending {} to {host} failed", tree.display())),
    }
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

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("reading {url} back failed: {error}"))?;
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

fn pin(common: &Path, build: &str, pins: &[(String, String)]) -> Result<(), String> {
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
