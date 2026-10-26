use beui_html::{elements as html, tw};
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
the new build, and pins the uploaded be in scripts/internal/pins.

The downloads are the app with its plugins for Linux, macOS and Windows on
x86_64 and arm64 and the web bundle, each zipped, and the APK, signed with the
release keystore under the id and label --android-id and --android-label give
(com.be3.block and Block), so it installs beside CI's com.be3.block.ci. They go
to data/BUILD/app/ beside a page linking them, and once the server runs the build,
web/downloads.html is that unlinked page. It is not
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
  BE3_DEPLOY_HOST     the server, for ssh and scp (ubuntu@api.be3.pfg.pw)
  BE3_DEPLOY_DIR      the directory there that holds be-server (be-server)
  BE3_DEPLOY_UNITS    the units stopped around the swap (be-server)
  Web files are uploaded under data/BUILD/web/; /index.html selects the live build.";

struct Options {
    apk_metadata: PathBuf,
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
    match parse(arguments.into_iter()).and_then(|mut options| run(&mut options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("deploy: {message}");
            ExitCode::FAILURE
        }
    }
}

fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        apk_metadata: PathBuf::new(),
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
            "--apk-metadata" => options.apk_metadata = value()?.into(),
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

fn secret(variable: &str, name: &str, description: &str) -> Result<String, String> {
    let saved = prompt::Saved {
        variable,
        name,
        what: variable,
        checkout: None,
    };
    if let Some(value) = saved.stored() {
        return Ok(value);
    }
    saved
        .ask(&format!("./scripts/deploy needs {description} (guides/hosting.md)."))
        .ok_or_else(|| {
            format!(
                "set {variable}, or write {description} to {}, or run ./scripts/deploy at a terminal to be asked for it.",
                saved.path().display()
            )
        })
}

fn setting(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn run(options: &mut Options) -> Result<(), String> {
    let mut build = None;
    for (triple, bundle) in &options.be {
        let id = read_build(&bundle.join("build-info.json"))?;
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
    if !options.skip_downloads && !options.dry_run {
        for (platform, file) in &mut options.downloads {
            if platform == "android" {
                if read_build(&options.apk_metadata)? != build {
                    return Err("APK metadata and native bundles have different build ids".into());
                }
                let key = crate::apk_sign::release_key()?;
                let output = options
                    .repository
                    .join("target/deploy/block-app-android.apk");
                crate::apk_sign::finalize(file, &output, &key, &options.apk_metadata)?;
                *file = output;
            }
        }
    }

    let storage = setting(
        "BE3_DEPLOY_STORAGE",
        "https://la.storage.bunnycdn.com/be-app",
    );
    let mirror = setting("BE3_DEPLOY_MIRROR", "https://be3.pfg.pw");
    let needs_storage = !options.dry_run
        && (!options.skip_be
            || !options.skip_downloads
            || !options.skip_web
            || !options.skip_server);
    let key = if needs_storage {
        secret(
            "BUNNY_STORAGE_KEY",
            "be-app-storage-key",
            "the Bunny storage zone's password",
        )?
    } else {
        String::new()
    };
    let web = options.web.as_ref().filter(|_| !options.skip_web);
    let publishes_pages = !options.skip_server
        && (web.is_some() || (!options.skip_downloads && !options.downloads.is_empty()));
    let api_key = if publishes_pages && !options.dry_run {
        secret(
            "BUNNY_API_KEY",
            "bunny-api-key",
            "the Bunny account's API key, to purge mutable entry points from the CDN",
        )?
    } else {
        String::new()
    };

    if let Some(web) = web {
        let id = read_build(&web.join("build-info.json"))?;
        if id != build {
            return Err(format!("the web bundle is build {id}, not {build}"));
        }
    }
    let web_entry = web.map(|_| web_entry_point(&build));
    let service_worker = web
        .map(|web| {
            let path = web.join("sw.js");
            std::fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
        })
        .transpose()?;
    let agents = Agents::new();
    let store = Store {
        options,
        agents: &agents,
        storage: &storage,
        mirror: &mirror,
        key: &key,
    };

    let mut pins = Vec::new();
    if !options.skip_be {
        for (triple, bundle) in &options.be {
            let directory = format!("data/{build}/be-{triple}");
            for name in ["be", "build-info.json"] {
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
            if platform == "android" && options.dry_run {
                println!(
                    "would finalize and sign the Android template locally, then upload block-app-android.apk"
                );
                continue;
            }
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
            "server build-info.json",
            &format!("data/{build}/server/build-info.json"),
            &std::fs::read(options.be[0].1.join(build_id::FILE))
                .map_err(|error| format!("cannot read build metadata: {error}"))?,
            "application/json",
            true,
        )?;
    }

    if !options.skip_server {
        let host = setting("BE3_DEPLOY_HOST", "ubuntu@api.be3.pfg.pw");
        let directory = setting("BE3_DEPLOY_DIR", "be-server");
        let units = setting("BE3_DEPLOY_UNITS", "be-server");
        let build_id = options
            .be
            .first()
            .map(|(_, bundle)| bundle.join("build-info.json"))
            .ok_or("no build-info.json to send")?;
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
            &[
                &build_id,
                &format!("{host}:{directory}/build-info.json.new"),
            ],
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
                    "chmod +x {directory}/be-server.new && mv {directory}/be-server.new {directory}/be-server && mv {directory}/build-info.json.new {directory}/build-info.json"
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
            "index.html",
            launch.as_bytes(),
            "text/html; charset=utf-8",
            false,
        )?;
        purge_pages(options, &agents, &api_key, &mirror, &["index.html", ""])?;
    }
    if let Some(worker) = service_worker.filter(|_| !options.skip_server) {
        store.put(
            "the web app's service worker",
            "sw.js",
            &worker,
            "text/javascript; charset=utf-8",
            false,
        )?;
        purge_pages(options, &agents, &api_key, &mirror, &["sw.js"])?;
    }

    if page.is_some() && options.skip_server {
        println!(
            "the server still runs its last build, so {mirror}/web/downloads.html still offers that one's \
             downloads; this build's are at {mirror}/data/{build}/app/"
        );
    } else if let Some(page) = page {
        store.put(
            "the latest download page",
            "web/downloads.html",
            page.as_bytes(),
            "text/html; charset=utf-8",
            false,
        )?;
        purge_pages(options, &agents, &api_key, &mirror, &["web/downloads.html"])?;
        println!("the downloads for build {build} are at {mirror}/web/downloads.html");
    }

    if !pins.is_empty() {
        let common = options.repository.join("scripts/internal/pins");
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
    use beui_html::{Children, Node, render, view};
    render(|| {
        let rows: Children<Node> = downloads
            .iter()
            .map(|download| {
                view! {
                    <html::Tr>
                        <html::Td>{download.label}</html::Td>
                        <html::Td>
                            <html::A href={format!("{prefix}{}", download.name)}>
                                {download.name.as_str()}
                            </html::A>
                        </html::Td>
                        <html::Td>
                            {format!("{:.1} MB", download.size as f64 / 1_000_000.0)}
                        </html::Td>
                        <html::Td>
                            <html::Code
                                css=<>
                                    <tw::TextXs />
                                    <tw::BreakAll />
                                </>
                            >
                                {download.sha256.as_str()}
                            </html::Code>
                        </html::Td>
                    </html::Tr>
                }
            })
            .collect();
        view! {
            <html::Html
                lang="en"
                css=<>
                    <tw::ColorSchemeLightDark />
                    <tw::FontSans />
                </>
            >
                <html::Head>
                    <html::Meta charset="utf-8" />
                    <html::Meta name="viewport" content="width=device-width, initial-scale=1" />
                    <html::Meta name="robots" content="noindex, nofollow" />
                    <html::Title>"Block downloads"</html::Title>
                    <tw::Styles />
                </html::Head>
                <html::Body
                    css=<>
                        <tw::MaxW240 />
                        <tw::My8 />
                        <tw::MxAuto />
                        <tw::Px4 />
                    </>
                >
                    <html::H1>"Block"</html::H1>
                    <html::P>
                        "Build "
                        <html::Code
                            css=<>
                                <tw::TextXs />
                                <tw::BreakAll />
                            </>
                        >
                            {build}
                        </html::Code>
                        ". A download connects only to a server running the same build."
                    </html::P>
                    <html::Table
                        css=<>
                            <tw::BorderCollapse />
                            <tw::WFull />
                            <tw::Descendants target=tw::Target::Cell>
                                <tw::TextLeft />
                                <tw::Padding size=tw::Spacing::Table />
                                <tw::AlignTop />
                            </tw::Descendants>
                        </>
                    >
                        <html::Tr>
                            <html::Th>"Platform"</html::Th>
                            <html::Th>"Download"</html::Th>
                            <html::Th>"Size"</html::Th>
                            <html::Th>"SHA-256"</html::Th>
                        </html::Tr>
                        {rows}
                    </html::Table>
                    <html::P>
                        "Each zip holds the app in a folder of its own: run "
                        <html::Code
                            css=<>
                                <tw::TextXs />
                                <tw::BreakAll />
                            </>
                        >
                            "block-app"
                        </html::Code>
                        " ("
                        <html::Code
                            css=<>
                                <tw::TextXs />
                                <tw::BreakAll />
                            </>
                        >
                            "block-app.exe"
                        </html::Code>
                        " on Windows) inside it. The macOS builds are not signed, so macOS asks before opening them; "
                        <html::Code
                            css=<>
                                <tw::TextXs />
                                <tw::BreakAll />
                            </>
                        >
                            "xattr -dr com.apple.quarantine block-app-macos-*"
                        </html::Code>
                        " lets them open. The web bundle is also available to serve yourself. "
                        <html::A href="https://be3.pfg.pw/">"Open the hosted web app"</html::A>
                        "."
                    </html::P>
                </html::Body>
            </html::Html>
        }
    })
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

fn web_entry_point(build: &str) -> String {
    crate::web_page::page(
        crate::web_page::Page::Block,
        Some(format!("/data/{build}/web/")),
    )
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
    key: &str,
    mirror: &str,
    paths: &[&str],
) -> Result<(), String> {
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
    let metadata = build_id::Metadata::parse(&id)?;
    if metadata.app_id != "Block" || metadata.default_server.is_none() {
        return Err(format!(
            "{} has no deployed app identity or default server; build through ./scripts/deploy",
            path.display()
        ));
    }
    Ok(metadata.id)
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
    set("be_metadata_file", "build-info.json")?;
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
