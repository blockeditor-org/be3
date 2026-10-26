use std::io::Read;
use std::process::ExitCode;

use be_block::BlockContent;
use be_cli::cli::{self, Outcome, USAGE};
use be_cli::session::{Settings, connect, connect_live};
use be_cli::terminal_host::{Host, HostOptions};
use be_cli::tracker::Tracker;

#[tokio::main]
async fn main() -> ExitCode {
    plugins();
    match run().await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("be: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<String, String> {
    let invocation = cli::parse(std::env::args().skip(1).collect(), || {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("could not read standard input: {error}"))?;
        Ok(text)
    })?;
    if invocation.target.is_none() && invocation.verb == "help" {
        return Ok(USAGE.to_owned());
    }
    build_id::current()?;
    let settings = Settings::from_env()?;
    if invocation.verb == "terminal-host"
        && let Some(target) = &invocation.target
    {
        return host_terminal(&settings, target, &invocation.options.command).await;
    }
    let tracker = Tracker::new(connect(&settings).await?, sources(&settings));
    let (block, label, base) = match cli::run(&tracker, &invocation).await? {
        Outcome::Print(text) => return Ok(text),
        Outcome::Edit { block, label, base } => (block, label, base),
        Outcome::Watch {
            root,
            directory,
            every,
        } => {
            let stop = async {
                let _ = tokio::signal::ctrl_c().await;
            };
            be_sync::watch::watch(
                tracker.peer(),
                &mut *tracker.types().await,
                &directory,
                &be_sync::Target { root },
                every,
                stop,
                |passed| match passed {
                    Ok(report) if report.changed() || report.retry => println!("{report}"),
                    Ok(_) => {}
                    Err(error) => eprintln!("be: {error}"),
                },
            )
            .await?;
            return Ok(String::new());
        }
    };
    let directory = private_directory()?;
    let path = directory.join(format!("{}.md", block.simple()));
    std::fs::write(&path, cli::edit_document(&label, &base))
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_owned());
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(&path)
        .status()
        .map_err(|error| format!("could not start {editor}: {error}"))?;
    let edited = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let _ = std::fs::remove_dir_all(&directory);
    if !status.success() {
        return Err(format!("{editor} failed, so nothing was saved"));
    }
    let body = cli::edited_body(&edited);
    if body == base {
        return Ok("unchanged".to_owned());
    }
    match tracker.rewrite_text(block, &base, &body).await? {
        true => Ok(format!("saved {label}")),
        false => Ok(format!(
            "saved {label}, with conflict markers where someone else changed the same lines"
        )),
    }
}

const PLUGIN_TYPES: &[u8] = include_bytes!(env!("PLUGIN_TYPES"));

fn plugins() -> &'static [be_cli::plugins::Plugin] {
    if be_cli::plugins::installed().is_empty() {
        match be_cli::plugins::parse(PLUGIN_TYPES) {
            Ok(plugins) => return be_cli::plugins::install(plugins),
            Err(error) => eprintln!("be: {error}"),
        }
    }
    be_cli::plugins::installed()
}

fn sources(settings: &Settings) -> be_cli::tracker::Sources {
    let cache = settings.cache.join("modules");
    let plugins = plugins();
    Box::new(move || {
        let mut sources = Vec::new();
        for plugin in plugins.iter().filter(|plugin| !plugin.module.is_empty()) {
            let module = plugin.module;
            sources.push(
                be_types_host::modules::wasm(
                    plugin.manifest.identity.id.clone(),
                    Ok(be_types_host::tag_of(module)),
                    Some(cache.clone()),
                    move |host| host.load_bytes(module),
                )
                .declaring(plugin.declared()),
            );
        }
        sources
    })
}

fn private_directory() -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join(format!("be-{}", uuid::Uuid::new_v4().simple()));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(&path)
        .map_err(|error| format!("could not make {}: {error}", path.display()))?;
    Ok(path)
}

async fn host_terminal(
    settings: &Settings,
    target: &str,
    command: &[String],
) -> Result<String, String> {
    let mut host = Host::new(HostOptions::here(command.to_vec()))?;
    let connect = async || {
        let tracker = Tracker::new(connect_live(settings).await?, sources(settings));
        let entry = tracker.resolve(target).await?;
        if entry.block_type != terminal_types::TerminalContent::CONTENT_TYPE {
            return Err(format!("{} is not a terminal block", entry.label()));
        }
        Ok((tracker.shared_peer(), entry.id))
    };
    let stopped = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    tokio::task::LocalSet::new()
        .run_until(host.run(connect, &sources(settings), stopped))
        .await?;
    Ok(String::new())
}
