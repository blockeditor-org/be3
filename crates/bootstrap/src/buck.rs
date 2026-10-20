use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead as _, BufReader, IsTerminal as _, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};

use crate::pins::Pins;
use crate::platform::target_platform;
use crate::quiet::Quiet;
use prompt::columns;

use crate::{Result, buck2, env_set, in_ci, repository, server, write_if_changed};

const BUILD_COMMANDS: [&str; 11] = [
    "build", "run", "test", "bxl", "install", "targets", "query", "cquery", "uquery", "aquery",
    "audit",
];

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let buck2 = prepare()?;
    let command = arguments.first().cloned().unwrap_or_default();
    if BUILD_COMMANDS.contains(&command.as_str()) {
        write_build_id()?;
    }
    let (status, output) = match command.as_str() {
        "test" => {
            let mut test: Vec<String> = vec![
                "test".into(),
                "--unstable-allow-compatible-tests-on-re".into(),
            ];
            test.extend(arguments.into_iter().skip(1));
            let quiet = !env_set("BE3_VERBOSE") && !in_ci();
            retrying(&buck2, &test, quiet, false, None)?
        }
        "run" => return run(&buck2, arguments),
        _ => retrying(&buck2, &arguments, false, false, None)?,
    };
    print_captured(&output);
    Ok(exit_code(status))
}

pub fn prepare() -> Result<PathBuf> {
    let pins = Pins::read()?;
    let buck2 = buck2::ensure(&pins)?;
    let target = repository().join("target");
    std::fs::create_dir_all(&target)
        .map_err(|error| format!("could not create {}: {error}", target.display()))?;
    server::configure(&buck2, false)?;
    Ok(buck2)
}

pub fn exit_code(status: i32) -> ExitCode {
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}

fn before_separator(arguments: &[String]) -> impl Iterator<Item = &String> {
    arguments.iter().take_while(|argument| *argument != "--")
}

fn run(buck2: &Path, mut arguments: Vec<String>) -> Result<ExitCode> {
    if let Some(host) = target_platform()
        && host != "linux_x86_64"
    {
        let named = before_separator(&arguments).any(|argument| {
            argument == "--target-platforms" || argument.starts_with("--target-platforms=")
        });
        if !named {
            arguments.splice(
                1..1,
                [
                    "--target-platforms".to_owned(),
                    format!("root//buck/platforms:{host}"),
                ],
            );
        }
    }
    let prebuild = !before_separator(&arguments).any(|argument| {
        argument == "--emit-shell"
            || argument == "--command-args-file"
            || argument.starts_with("--command-args-file=")
    });
    if prebuild {
        let mut build = vec!["run".to_owned(), "--command-args-file".to_owned()];
        build.push(
            repository()
                .join("target/run-command-args")
                .to_string_lossy()
                .into_owned(),
        );
        build.extend(arguments.iter().skip(1).cloned());
        let (status, output) = retrying(buck2, &build, false, false, None)?;
        print_captured(&output);
        if status != 0 {
            return Ok(exit_code(status));
        }
    }
    let mut command = Command::new(buck2);
    command.args(&arguments);
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        command.env("RUST_BACKTRACE", "1");
    }
    exec(command)
}

pub fn exec(mut command: Command) -> Result<ExitCode> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let error = command.exec();
        Err(format!(
            "could not run {}: {error}",
            command.get_program().to_string_lossy()
        ))
    }
    #[cfg(not(unix))]
    {
        let status = command.status().map_err(|error| {
            format!(
                "could not run {}: {error}",
                command.get_program().to_string_lossy()
            )
        })?;
        Ok(exit_code(status_code(status)))
    }
}

pub fn status_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

pub fn retrying(
    buck2: &Path,
    arguments: &[String],
    quiet: bool,
    capture: bool,
    stderr: Option<&Path>,
) -> Result<(i32, Vec<u8>)> {
    let attempts = match std::env::var("BE3_BUCK_ATTEMPTS") {
        Err(_) => 3,
        Ok(value) => match value.parse::<u32>() {
            Ok(attempts) if (1..=99).contains(&attempts) && !value.starts_with('0') => attempts,
            _ => {
                return Err(format!(
                    "BE3_BUCK_ATTEMPTS is a number of attempts from 1 to 99, not '{value}'."
                ));
            }
        },
    };
    let capture = capture || !std::io::stdout().is_terminal();
    let mut attempt = 1;
    loop {
        let (status, output) = attempt_once(buck2, arguments, quiet, capture, stderr)?;
        if status != 2 || attempt >= attempts {
            return Ok((status, output));
        }
        attempt += 1;
        eprintln!(
            "buck2 failed with an infrastructure error; trying again ({attempt} of {attempts}). \
             guides/build_server.md says what to do when this keeps happening."
        );
        server::configure(buck2, true)?;
    }
}

pub fn print_captured(output: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(output);
    let _ = stdout.flush();
}

fn attempt_once(
    buck2: &Path,
    arguments: &[String],
    quiet: bool,
    capture: bool,
    stderr: Option<&Path>,
) -> Result<(i32, Vec<u8>)> {
    let mut command = Command::new(buck2);
    command.args(arguments.iter().map(OsString::from));
    let spawn_error = |error: std::io::Error| format!("could not run {}: {error}", buck2.display());
    let mut output = Vec::new();
    if quiet {
        let (reader, writer) =
            std::io::pipe().map_err(|error| format!("could not open a pipe: {error}"))?;
        let duplicate = writer
            .try_clone()
            .map_err(|error| format!("could not open a pipe: {error}"))?;
        command.stdout(writer).stderr(duplicate);
        let mut child = command.spawn().map_err(spawn_error)?;
        drop(command);
        let mut filter = Quiet::new(!capture, columns());
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let mut stdout = std::io::stdout();
        let out: &mut dyn Write = if capture { &mut output } else { &mut stdout };
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&line);
                    filter.line(text.trim_end_matches(['\n', '\r']), out);
                }
            }
        }
        filter.finish(out);
        let status = child.wait().map_err(spawn_error)?;
        return Ok((status_code(status), output));
    }
    if let Some(log) = stderr {
        let file = File::create(log)
            .map_err(|error| format!("could not write {}: {error}", log.display()))?;
        command.stderr(file);
    }
    if capture {
        command.stdout(Stdio::piped());
    }
    let mut child = command.spawn().map_err(spawn_error)?;
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut output);
    }
    let status = child.wait().map_err(spawn_error)?;
    Ok((status_code(status), output))
}

pub fn write_build_id() -> Result<()> {
    let path = repository().join("buck/app/build-info.json");
    let id = match std::env::var("BE3_BUILD_ID") {
        Ok(id) if !id.is_empty() => id,
        _ => build_id(),
    };
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
    {
        return Err("Invalid BE3_BUILD_ID".to_owned());
    }
    let app_id = std::env::var("BE3_APP_ID")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "BlockDev".to_owned());
    if app_id != "Block" && app_id != "BlockDev" {
        return Err("Invalid BE3_APP_ID".to_owned());
    }
    let text = if std::env::var("BE3_DEPLOY_DEFAULT_SERVER").is_ok_and(|value| value == "true") {
        format!(
            "{{\"id\":\"{id}\",\"app_id\":\"{app_id}\",\"default_server\":{{\"name\":\"Block\",\"api_url\":\"https://api.be3.pfg.pw\",\"auth_url\":\"https://api.be3.pfg.pw/auth\",\"gateway_required\":true}}}}\n"
        )
    } else {
        format!("{{\"id\":\"{id}\",\"app_id\":\"{app_id}\"}}\n")
    };
    write_if_changed(&path, text.as_bytes())?;
    Ok(())
}

fn build_id() -> String {
    let repository = repository();
    let git = |index: &Path, arguments: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env("GIT_INDEX_FILE", index)
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let index = std::env::temp_dir().join(format!("be3-build-id-{}", std::process::id()));
    let _ = std::fs::remove_file(&index);
    let real = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "--path-format=absolute", "--git-path", "index"])
        .stderr(Stdio::null())
        .output()
        .ok()
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()));
    if let Some(real) = real {
        let _ = std::fs::copy(real, &index);
    }
    let tree = git(&index, &["add", "-A"]).and_then(|_| git(&index, &["write-tree"]));
    let _ = std::fs::remove_file(&index);
    tree.filter(|tree| !tree.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
