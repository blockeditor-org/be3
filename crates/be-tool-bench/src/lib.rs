use std::io::Write;
use std::time::Duration;

pub const METRICS: &str = "BENCH_METRICS";

pub mod suite;

pub fn record(name: &str, value: f64, unit: &str) {
    let Ok(path) = std::env::var(METRICS) else {
        return;
    };
    let line = format!("{name}\t{value}\t{unit}\n");
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(line.as_bytes()));
    if let Err(error) = written {
        panic!("could not write {name} to {path}: {error}");
    }
}

pub fn time(name: &str, taken: Duration) {
    record(name, taken.as_secs_f64() * 1000.0, "ms");
}

pub fn count(name: &str, count: u64) {
    record(name, count as f64, "");
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

pub fn parse(text: &str) -> Result<Vec<Metric>, String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(name), Some(value), Some(unit), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return Err(format!("{line:?} is not NAME, VALUE and UNIT"));
            };
            let value = value
                .parse()
                .map_err(|_| format!("{value:?} in {line:?} is not a number"))?;
            Ok(Metric {
                name: name.to_owned(),
                value,
                unit: unit.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
