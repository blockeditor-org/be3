use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{METRICS, Metric};

pub struct Benchmark {
    pub name: &'static str,
    pub program: &'static str,
    pub arguments: &'static [&'static str],
    pub about: &'static str,
}

pub const BENCHMARKS: &[Benchmark] = &[
    Benchmark {
        name: "app-canvas-and-text",
        program: "app-bench",
        arguments: &[],
        about: "block-app end to end against a server 30 ms away: canvas and text blocks made, saved, painted, closed, exported and reopened on a new device",
    },
    Benchmark {
        name: "canvas",
        program: "canvas-bench",
        arguments: &[],
        about: "the canvas editor opening, panning, zooming, dragging and inking 100 and 10,000 shapes",
    },
    Benchmark {
        name: "open",
        program: "open-bench",
        arguments: &[],
        about: "opening each beui-demo page, cold and warm",
    },
    Benchmark {
        name: "text",
        program: "text-bench",
        arguments: &[],
        about: "resizing and editing beui text",
    },
    Benchmark {
        name: "access",
        program: "access-bench",
        arguments: &[],
        about: "the server's access queries on an organization of 100,000 blocks",
    },
];

pub struct Suite {
    pub dir: PathBuf,
}

impl Suite {
    pub fn at(dir: &Path) -> Result<Self, String> {
        let programs = dir.join("programs");
        if !programs.is_dir() {
            return Err(format!(
                "{} is not a benchmark suite: it has no programs/; build one with ./scripts/buck build //crates/bench:suite --out DIR",
                dir.display()
            ));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn run(
        &self,
        benchmark: &Benchmark,
        log: &Path,
        verbose: bool,
    ) -> Result<Vec<Metric>, String> {
        let metrics = log.with_extension("metrics");
        let _ = std::fs::remove_file(&metrics);
        let mut command = Command::new(self.dir.join("programs").join(benchmark.program));
        command
            .args(benchmark.arguments)
            .env(METRICS, &metrics)
            .env("BLOCK_APP", self.dir.join("block-app").join("block-app"))
            .env("APP_LIBRARIES", self.dir.join("lib"))
            .stdin(Stdio::null());
        if !verbose {
            let file = std::fs::File::create(log)
                .map_err(|error| format!("could not create {}: {error}", log.display()))?;
            let copy = file.try_clone().map_err(|error| error.to_string())?;
            command.stdout(file).stderr(copy);
        }
        let status = command
            .status()
            .map_err(|error| format!("could not start {}: {error}", benchmark.program))?;
        if !status.success() {
            let output = std::fs::read_to_string(log).unwrap_or_default();
            let tail: Vec<&str> = output.lines().rev().take(60).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(format!(
                "{} failed ({status}); the end of its output, all of it in {}:\n{}",
                benchmark.name,
                log.display(),
                tail.join("\n")
            ));
        }
        let text = std::fs::read_to_string(&metrics).map_err(|error| {
            format!(
                "{} reported no metrics in {}: {error}",
                benchmark.name,
                metrics.display()
            )
        })?;
        crate::parse(&text)
    }
}

#[derive(Default)]
pub struct Samples {
    pub order: Vec<String>,
    pub units: BTreeMap<String, String>,
    pub values: BTreeMap<String, Vec<f64>>,
}

impl Samples {
    pub fn add(&mut self, prefix: &str, metrics: Vec<Metric>) {
        for metric in metrics {
            let name = format!("{prefix}/{}", metric.name);
            if !self.values.contains_key(&name) {
                self.order.push(name.clone());
            }
            self.units.insert(name.clone(), metric.unit);
            self.values.entry(name).or_default().push(metric.value);
        }
    }
}

pub fn fastest(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::INFINITY, f64::min)
}

pub fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    match sorted.len() {
        0 => f64::NAN,
        length if length % 2 == 1 => sorted[length / 2],
        length => (sorted[length / 2 - 1] + sorted[length / 2]) / 2.0,
    }
}

pub fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(column, cell)| match column {
                    0 => format!("{cell:<width$}", width = widths[column]),
                    _ => format!("{cell:>width$}", width = widths[column]),
                })
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn number(value: f64, unit: &str) -> String {
    let digits = match value.abs() {
        size if size >= 100.0 => 0,
        size if size >= 10.0 => 1,
        _ => 2,
    };
    match unit {
        "" => format!("{value:.digits$}"),
        unit => format!("{value:.digits$} {unit}"),
    }
}
