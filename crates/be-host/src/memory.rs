//! be-host's memory statistics, on Linux under `--desktop`: a sample every ten seconds, kept as a
//! point a minute (the peak of each measurement through it) for two days, drawn in the
//! Performance panel and written to `memory.tsv` in the state directory, beside the login
//! session's log. The previous run's file is kept as `memory.previous.tsv`.

use std::collections::{BTreeMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use be_host_base::performance::PerformanceRow;

use crate::ui::{MemoryLine, MemoryPoint, MemoryRow, MemoryUsage, MemoryView};

const INTERVAL: Duration = Duration::from_secs(10);
/// Two days of minutes.
const MINUTES: usize = 48 * 60;
const FILE: &str = "memory.tsv";
const PREVIOUS: &str = "memory.previous.tsv";
const HEADER: &str = "time\tresident_kib\tanonymous_kib\tfile_kib\tshared_kib\theap_kib\theap_free_kib\twasm_kib\twasm_by_guest_kib";

/// One sample: when, in seconds since the Unix epoch, the process's memory, and each wasm
/// guest's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) at: u64,
    pub(crate) usage: MemoryUsage,
    pub(crate) guests: Vec<(String, u64)>,
}

/// A minute that has ended, with the peak of each guest through it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Minute {
    pub(crate) point: MemoryPoint,
    pub(crate) guests: BTreeMap<String, u64>,
}

#[derive(Default)]
pub(crate) struct History {
    minutes: VecDeque<MemoryPoint>,
    open: Option<Minute>,
    latest: Option<Sample>,
    guest_peaks: BTreeMap<String, u64>,
    generation: u64,
    file: Option<PathBuf>,
}

impl History {
    /// Takes a sample in, and gives back the minute it ended, if it began another.
    pub(crate) fn record(&mut self, sample: Sample) -> Option<Minute> {
        let minute = sample.at / 60 * 60;
        let ended = match self.open.take() {
            Some(mut open) if open.point.at == minute => {
                open.point.usage = open.point.usage.peak(sample.usage);
                peak_guests(&mut open.guests, &sample.guests);
                self.open = Some(open);
                None
            }
            ended => {
                let mut guests = BTreeMap::new();
                peak_guests(&mut guests, &sample.guests);
                self.open = Some(Minute {
                    point: MemoryPoint {
                        at: minute,
                        usage: sample.usage,
                    },
                    guests,
                });
                ended
            }
        };
        if let Some(ended) = &ended {
            if self.minutes.len() == MINUTES {
                self.minutes.pop_front();
            }
            self.minutes.push_back(ended.point);
        }
        peak_guests(&mut self.guest_peaks, &sample.guests);
        self.latest = Some(sample);
        self.generation += 1;
        ended
    }

    pub(crate) fn view(&self) -> Option<MemoryView> {
        let latest = self.latest.as_ref()?;
        let points: Arc<[MemoryPoint]> = self
            .minutes
            .iter()
            .copied()
            .chain(self.open.as_ref().map(|open| open.point))
            .collect();
        let mut rows = vec![heading("Memory")];
        let measurements: [(&str, Option<MemoryLine>, fn(&MemoryUsage) -> u64); 7] = [
            ("Resident", Some(MemoryLine::Resident), |usage| {
                usage.resident
            }),
            ("Anonymous", Some(MemoryLine::Anonymous), |usage| {
                usage.anonymous
            }),
            ("File-backed", None, |usage| usage.file),
            (
                "Shared (window buffers)",
                Some(MemoryLine::Shared),
                |usage| usage.shared,
            ),
            ("malloc heap", None, |usage| usage.heap),
            ("malloc heap, free", None, |usage| usage.heap_free),
            ("wasm memories", Some(MemoryLine::Wasm), |usage| usage.wasm),
        ];
        for (name, line, read) in measurements {
            let total: u64 = points.iter().map(|point| read(&point.usage)).sum();
            let average = total / points.len().max(1) as u64;
            let peak = points
                .iter()
                .map(|point| read(&point.usage))
                .max()
                .unwrap_or_default();
            rows.push(MemoryRow {
                row: PerformanceRow {
                    heading: false,
                    name: name.to_owned(),
                    current: bytes(read(&latest.usage)),
                    average: bytes(average),
                    peak: bytes(peak),
                },
                line,
            });
        }
        if !latest.guests.is_empty() {
            rows.push(heading("wasm memory by guest"));
        }
        for (name, current) in &latest.guests {
            rows.push(MemoryRow {
                row: PerformanceRow {
                    heading: false,
                    name: name.clone(),
                    current: bytes(*current),
                    average: "-".to_owned(),
                    peak: bytes(self.guest_peaks.get(name).copied().unwrap_or(*current)),
                },
                line: None,
            });
        }
        Some(MemoryView {
            generation: self.generation,
            points,
            now: latest.at,
            rows,
            file: self.file.as_ref().map(|file| file.display().to_string()),
        })
    }
}

fn peak_guests(peaks: &mut BTreeMap<String, u64>, guests: &[(String, u64)]) {
    for (name, bytes) in guests {
        let peak = peaks.entry(name.clone()).or_default();
        *peak = (*peak).max(*bytes);
    }
}

fn heading(name: &str) -> MemoryRow {
    MemoryRow {
        row: PerformanceRow {
            heading: true,
            name: name.to_owned(),
            current: String::new(),
            average: String::new(),
            peak: String::new(),
        },
        line: None,
    }
}

pub(crate) fn bytes(count: u64) -> String {
    format!("{:7.1} MiB", count as f64 / f64::from(1 << 20))
}

fn history() -> MutexGuard<'static, History> {
    static HISTORY: OnceLock<Mutex<History>> = OnceLock::new();
    HISTORY
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|held| held.into_inner())
}

static WATCHED: AtomicBool = AtomicBool::new(false);

/// Whether a Performance panel is open, which each sample then wakes the host to redraw.
pub(crate) fn watch(watched: bool) {
    WATCHED.store(watched, Ordering::Relaxed);
}

pub(crate) fn view() -> Option<MemoryView> {
    history().view()
}

/// Starts sampling, once a process.
pub(crate) fn start() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let log = match crate::login_session::state_directory() {
        Some(directory) => match Log::open(&directory) {
            Ok(log) => Some(log),
            Err(error) => {
                eprintln!("be-host: memory statistics are not written to a file: {error}");
                None
            }
        },
        None => None,
    };
    history().file = log.as_ref().map(|log| log.path.clone());
    let started = std::thread::Builder::new()
        .name("memory statistics".to_owned())
        .spawn(move || run(log));
    if let Err(error) = started {
        eprintln!("be-host: memory statistics did not start: {error}");
    }
}

fn run(mut log: Option<Log>) {
    loop {
        let ended = history().record(sample());
        if let (Some(ended), Some(writing)) = (ended, log.as_mut())
            && let Err(error) = writing.write(&ended)
        {
            eprintln!("be-host: memory statistics stopped being written: {error}");
            log = None;
        }
        if WATCHED.load(Ordering::Relaxed) {
            be_host_base::host::wake();
        }
        std::thread::sleep(INTERVAL);
    }
}

fn sample() -> Sample {
    let guests = be_host_runtime::wasm_memories();
    let mut usage = std::fs::read_to_string("/proc/self/status")
        .map(|status| from_status(&status))
        .unwrap_or_default();
    usage.wasm = guests.iter().map(|(_, bytes)| bytes).sum();
    (usage.heap, usage.heap_free) = heap();
    Sample {
        at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        usage,
        guests,
    }
}

/// Reads resident memory from `/proc/self/status`, whose `RssAnon`, `RssFile` and `RssShmem`
/// sum to `VmRSS`.
pub(crate) fn from_status(status: &str) -> MemoryUsage {
    let mut usage = MemoryUsage::default();
    for line in status.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let field = match name {
            "VmRSS" => &mut usage.resident,
            "RssAnon" => &mut usage.anonymous,
            "RssFile" => &mut usage.file,
            "RssShmem" => &mut usage.shared,
            _ => continue,
        };
        let kib = value.trim().trim_end_matches("kB").trim();
        *field = kib.parse::<u64>().unwrap_or_default() * 1024;
    }
    usage
}

#[cfg(target_env = "gnu")]
fn heap() -> (u64, u64) {
    // mallinfo2 takes nothing and returns a struct by value.
    // unsafe(ffi)
    let info = unsafe { libc::mallinfo2() };
    ((info.arena + info.hblkhd) as u64, info.fordblks as u64)
}

#[cfg(not(target_env = "gnu"))]
fn heap() -> (u64, u64) {
    (0, 0)
}

struct Log {
    path: PathBuf,
    file: File,
    lines: usize,
}

impl Log {
    fn open(directory: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(directory)?;
        let path = directory.join(FILE);
        let _ = std::fs::rename(&path, directory.join(PREVIOUS));
        let file = create(&path)?;
        let mut log = Self {
            path,
            file,
            lines: 0,
        };
        writeln!(log.file, "{HEADER}")?;
        Ok(log)
    }

    fn write(&mut self, minute: &Minute) -> io::Result<()> {
        writeln!(self.file, "{}", line(minute))?;
        self.lines += 1;
        if self.lines >= 2 * MINUTES {
            self.trim()?;
        }
        Ok(())
    }

    /// Keeps the file to the last two days, rewriting it once it holds four.
    fn trim(&mut self) -> io::Result<()> {
        let mut kept: Vec<String> = BufReader::new(File::open(&self.path)?)
            .lines()
            .skip(1)
            .collect::<io::Result<_>>()?;
        let kept = kept.split_off(kept.len().saturating_sub(MINUTES));
        let fresh = self.path.with_extension("tsv.new");
        let mut file = create(&fresh)?;
        writeln!(file, "{HEADER}")?;
        for line in &kept {
            writeln!(file, "{line}")?;
        }
        std::fs::rename(&fresh, &self.path)?;
        self.file = file;
        self.lines = kept.len();
        Ok(())
    }
}

fn create(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

/// A minute as a line of `memory.tsv`: sizes in KiB, and each guest as `name=KiB`, split by `;`.
pub(crate) fn line(minute: &Minute) -> String {
    let usage = minute.point.usage;
    let guests: Vec<String> = minute
        .guests
        .iter()
        .map(|(name, bytes)| format!("{name}={}", bytes / 1024))
        .collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        minute.point.at,
        usage.resident / 1024,
        usage.anonymous / 1024,
        usage.file / 1024,
        usage.shared / 1024,
        usage.heap / 1024,
        usage.heap_free / 1024,
        usage.wasm / 1024,
        guests.join(";")
    )
}

#[cfg(test)]
mod tests;
