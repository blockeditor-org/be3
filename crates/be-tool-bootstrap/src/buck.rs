use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead as _, BufReader, IsTerminal as _, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};

use crate::pins::Pins;
use crate::platform::target_platform;
use crate::quiet::Quiet;
use be_lib_prompt::columns;

use crate::{Result, buck2, env_set, in_ci, repository, server, write_if_changed};

/// The commands that build, before which buck/program/build-info.json is
/// brought up to date with the working tree. A query reads no file's contents,
/// so before one it is only written if missing: buck2 cannot load
/// //buck/program without it.
const BUILD_COMMANDS: [&str; 5] = ["build", "run", "test", "bxl", "install"];

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let buck2 = prepare()?;
    let command = arguments.first().cloned().unwrap_or_default();
    if BUILD_COMMANDS.contains(&command.as_str()) || !repository().join(BUILD_INFO).exists() {
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
    server::configure(&buck2)?;
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
    let passthrough = before_separator(&arguments).any(|argument| {
        argument == "--emit-shell"
            || argument == "--command-args-file"
            || argument.starts_with("--command-args-file=")
    });
    if passthrough {
        let (status, output) = retrying(buck2, &arguments, false, false, None)?;
        print_captured(&output);
        return Ok(exit_code(status));
    }
    // buck2 cannot write the command to a file and change its directory in one
    // run, so the directory is taken here.
    let mut chdir = None;
    let mut build = vec!["run".to_owned(), "--command-args-file".to_owned()];
    let file = repository().join("target/run-command-args");
    build.push(file.to_string_lossy().into_owned());
    let mut rest = arguments.into_iter().skip(1);
    while let Some(argument) = rest.next() {
        if argument == "--" {
            build.push(argument);
            build.extend(rest.by_ref());
            break;
        }
        if argument == "--chdir" {
            chdir = rest.next();
        } else if let Some(directory) = argument.strip_prefix("--chdir=") {
            chdir = Some(directory.to_owned());
        } else {
            build.push(argument);
        }
    }
    let (status, output) = retrying(buck2, &build, false, false, None)?;
    print_captured(&output);
    if status != 0 {
        return Ok(exit_code(status));
    }
    let mut command = run_command(&file)?;
    if let Some(directory) = chdir {
        command.current_dir(directory);
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        command.env("RUST_BACKTRACE", "1");
    }
    exec(command)
}

/// The command `buck2 run --command-args-file` wrote: the program, its whole
/// argv and the environment buck2 would run it with, in this directory.
pub fn run_command(file: &Path) -> Result<Command> {
    let text = std::fs::read_to_string(file)
        .map_err(|error| format!("could not read {}: {error}", file.display()))?;
    let invalid = || format!("{} is not the command buck2 run writes", file.display());
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| invalid())?;
    let path = value["path"].as_str().ok_or_else(invalid)?;
    let argv = value["argv"].as_array().ok_or_else(invalid)?;
    let envp = value["envp"].as_object().ok_or_else(invalid)?;
    let mut command = Command::new(path);
    for argument in argv.iter().skip(1) {
        command.arg(argument.as_str().ok_or_else(invalid)?);
    }
    #[cfg(unix)]
    if let Some(first) = argv.first().and_then(|first| first.as_str()) {
        use std::os::unix::process::CommandExt as _;
        command.arg0(first);
    }
    command.env_clear();
    for (name, value) in envp {
        command.env(name, value.as_str().ok_or_else(invalid)?);
    }
    Ok(command)
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
        server::configure(buck2)?;
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

const BUILD_INFO: &str = "buck/program/build-info.json";

pub fn write_build_id() -> Result<()> {
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
    let program_id = std::env::var("BE3_PROGRAM_ID")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "be-host-dev".to_owned());
    if program_id != "be-host" && program_id != "be-host-dev" {
        return Err("Invalid BE3_PROGRAM_ID".to_owned());
    }
    let text = if std::env::var("BE3_DEPLOY_DEFAULT_SERVER").is_ok_and(|value| value == "true") {
        format!(
            "{{\"id\":\"{id}\",\"program_id\":\"{program_id}\",\"default_server\":{{\"name\":\"be3\",\"api_url\":\"https://api.be3.pfg.pw\",\"auth_url\":\"https://api.be3.pfg.pw/auth\",\"gateway_required\":true}}}}\n"
        )
    } else {
        format!("{{\"id\":\"{id}\",\"program_id\":\"{program_id}\"}}\n")
    };
    write_if_changed(&repository().join(BUILD_INFO), text.as_bytes())?;
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
