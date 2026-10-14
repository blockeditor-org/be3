use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Duration,
};

use sha2::{Digest, Sha256};

const USAGE: &str = "\
usage: ./scripts/deploy [--dry-run] [--allow-dirty] [--skip-server] [--skip-be] [--skip-web]

Builds be-server, the web bundle and be in release, uploads each be to the
mirror, swaps the server and the web bundle on the VPS, and pins the uploaded
be in scripts/internal/common.sh.

  --dry-run      print what it would do, uploading and changing nothing
  --allow-dirty  deploy a working tree that differs from its commit
  --skip-server  only upload be and pin it
  --skip-be      leave be as it is
  --skip-web     leave the web bundle as it is

Environment:
  BUNNY_STORAGE_KEY   the storage zone's password (needed to upload be)
  BE3_DEPLOY_STORAGE  where be goes (https://la.storage.bunnycdn.com/clfs/by-name/be3)
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

    let mut pins = Vec::new();
    if !options.skip_be {
        let storage = setting(
            "BE3_DEPLOY_STORAGE",
            "https://la.storage.bunnycdn.com/clfs/by-name/be3",
        );
        let mirror = setting("BE3_DEPLOY_MIRROR", "https://lfs.pfg.pw/by-name/be3");
        let key = std::env::var("BUNNY_STORAGE_KEY").unwrap_or_default();
        if key.trim().is_empty() && !options.dry_run {
            return Err("set BUNNY_STORAGE_KEY to the storage zone's password".to_owned());
        }
        let agents = Agents::new();
        for (triple, bundle) in &options.be {
            let directory = format!("be/{build}/be-{triple}");
            for name in ["be", "build-id"] {
                let bytes = std::fs::read(bundle.join(name)).map_err(|error| {
                    format!("cannot read {}: {error}", bundle.join(name).display())
                })?;
                let sha256 = hex(&Sha256::digest(&bytes));
                let target = format!("{storage}/{directory}/{name}");
                println!(
                    "upload {} ({} bytes, sha256 {sha256}) to {target}",
                    bundle.join(name).display(),
                    bytes.len()
                );
                if !options.dry_run {
                    upload(agents.to(&target), &target, key.trim(), &bytes, &sha256)?;
                    let source = format!("{mirror}/{directory}/{name}");
                    let read = download(agents.to(&source), &source)?;
                    if hex(&Sha256::digest(&read)) != sha256 {
                        return Err(format!(
                            "{source} is not what was uploaded to {target}; check \
                             BE3_DEPLOY_STORAGE and BE3_DEPLOY_MIRROR name the same place"
                        ));
                    }
                    println!("  read back from {source}");
                }
                if name == "be" {
                    pins.push((triple.clone(), sha256));
                }
            }
        }
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
) -> Result<(), String> {
    agent
        .put(url)
        .set("AccessKey", key)
        .set("Checksum", &sha256.to_ascii_uppercase())
        .set("Content-Type", "application/octet-stream")
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
