use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::{env, fs};

const USAGE: &str = "usage: fuzz-runner (--all | --list FILE) [LIBFUZZER-FLAG... | INPUT...]";

const SIGNALS: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

static GROUP: AtomicI32 = AtomicI32::new(0);
static STOPPED: AtomicBool = AtomicBool::new(false);

struct Target {
    id: String,
    binary: PathBuf,
}

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<ExitCode, String> {
    let (targets, arguments) = match arguments.first().map(String::as_str) {
        Some("--all") => (all()?, &arguments[1..]),
        Some("--list") if arguments.len() > 1 => {
            let list = fs::read_to_string(&arguments[1])
                .map_err(|error| format!("could not read {}: {error}", arguments[1]))?;
            (parse(&list)?, &arguments[2..])
        }
        _ => return Err(USAGE.to_owned()),
    };
    if targets.is_empty() {
        return Err("There are no fuzz targets.".to_owned());
    }
    if let Some(input) = arguments.iter().find(|argument| !argument.starts_with('-')) {
        let [target] = targets.as_slice() else {
            return Err(format!(
                "{input} is an input to run once, which takes one fuzz target, such as ./scripts/buck run //crates/sequence:fuzz-sequence."
            ));
        };
        let error = Command::new(&target.binary).args(arguments).exec();
        return Err(format!(
            "could not run {}: {error}",
            target.binary.display()
        ));
    }
    for signal in SIGNALS {
        unsafe {
            libc::signal(
                signal,
                forward as extern "C" fn(libc::c_int) as libc::sighandler_t,
            );
        }
    }
    let root = PathBuf::from(env::var_os("BE3_FUZZ_DIR").unwrap_or_else(|| "target/fuzz".into()));
    let jobs = std::thread::available_parallelism().map_or(1, |cores| cores.get());
    let turn = match targets.len() {
        1 => None,
        _ => Some(env::var("BE3_FUZZ_SLICE").unwrap_or_else(|_| "600".to_owned())),
    };
    loop {
        for target in &targets {
            let dir = root.join(&target.id);
            let corpus = dir.join("corpus");
            let crashes = dir.join("crashes");
            for dir in [&corpus, &crashes] {
                fs::create_dir_all(dir)
                    .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
            }
            eprintln!("Fuzzing {}.", target.id);
            let mut command = Command::new(&target.binary);
            command
                .arg(format!("-fork={jobs}"))
                .arg("-ignore_crashes=1")
                .arg(format!("-artifact_prefix={}/", crashes.display()));
            if let Some(turn) = &turn {
                command.arg(format!("-max_total_time={turn}"));
            }
            let mut child = command
                .args(arguments)
                .arg(&corpus)
                .stdin(Stdio::null())
                .process_group(0)
                .spawn()
                .map_err(|error| format!("could not run {}: {error}", target.binary.display()))?;
            let group = child.id() as i32;
            GROUP.store(group, Ordering::SeqCst);
            if STOPPED.load(Ordering::SeqCst) {
                unsafe {
                    libc::kill(-group, libc::SIGTERM);
                }
            }
            let waited = child.wait();
            GROUP.store(0, Ordering::SeqCst);
            waited.map_err(|error| format!("could not wait for {}: {error}", target.id))?;
            if STOPPED.load(Ordering::SeqCst) {
                return Ok(ExitCode::from(130));
            }
        }
        if turn.is_none() {
            return Ok(ExitCode::SUCCESS);
        }
    }
}

extern "C" fn forward(signal: libc::c_int) {
    STOPPED.store(true, Ordering::SeqCst);
    let group = GROUP.load(Ordering::SeqCst);
    if group > 0 {
        unsafe {
            libc::kill(-group, signal);
        }
    }
}

fn all() -> Result<Vec<Target>, String> {
    let output = Command::new("./scripts/buck")
        .args(["bxl", "//buck/dev/fuzz.bxl:targets"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not run ./scripts/buck: {error}"))?;
    if !output.status.success() {
        return Err("Building the fuzz targets failed.".to_owned());
    }
    parse(&String::from_utf8_lossy(&output.stdout))
}

fn parse(list: &str) -> Result<Vec<Target>, String> {
    list.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (id, binary) = line
                .split_once('\t')
                .ok_or_else(|| format!("{line:?} is not a fuzz target's id and binary"))?;
            Ok(Target {
                id: id.to_owned(),
                binary: PathBuf::from(binary),
            })
        })
        .collect()
}
