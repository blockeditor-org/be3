use std::io::Read;
use std::process::ExitCode;

use be_block::BlockContent;
use be_cli::cli::{self, Outcome, USAGE};
use be_cli::session::{Settings, connect, connect_live};
use be_cli::terminal_host::{Host, HostOptions};
use be_cli::tracker::Tracker;

#[tokio::main]
async fn main() -> ExitCode {
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
    let tracker = Tracker::new(connect(&settings).await?);
    let (block, label, base) = match cli::run(&tracker, &invocation).await? {
        Outcome::Print(text) => return Ok(text),
        Outcome::Edit { block, label, base } => (block, label, base),
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

async fn host_terminal(settings: &Settings, target: &str, command: &[String]) -> Result<String, String> {
    let mut host = Host::new(HostOptions::here(command.to_vec()))?;
    let connect = async || {
        let tracker = Tracker::new(connect_live(settings).await?);
        let entry = tracker.resolve(target).await?;
        if entry.block_type != be_block::TerminalContent::CONTENT_TYPE {
            return Err(format!("{} is not a terminal block", entry.label()));
        }
        Ok((tracker.shared_peer(), entry.id))
    };
    let stopped = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    host.run(connect, stopped).await?;
    Ok(String::new())
}
