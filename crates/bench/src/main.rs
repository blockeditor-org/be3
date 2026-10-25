use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bench::suite::{BENCHMARKS, Benchmark, Samples, Suite, fastest, median, number, table};

const USAGE: &str = "usage: bench list
       bench [run] [--rounds=N] [--verbose] [NAME...]
       bench compare BASE [--rounds=N] [--verbose] [NAME...]
Runs the benchmarks of the suite this program is in (guides/performance.md), each NAME
picking the one it names or else those whose name contains it, ROUNDS times each (3 by default). compare runs
them alternately from the suite at BASE and from this one, and prints each metric's
fastest and median from both, and how much this suite's fastest differs from BASE's.";

struct Options {
    compare: Option<PathBuf>,
    rounds: usize,
    verbose: bool,
    names: Vec<String>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let (command, rest) = match arguments.first().map(String::as_str) {
        Some(command @ ("list" | "run" | "compare")) => (command, &arguments[1..]),
        _ => ("run", arguments),
    };
    if command == "list" {
        for benchmark in BENCHMARKS {
            println!("{:<22}{}", benchmark.name, benchmark.about);
        }
        return Ok(());
    }
    let options = parse(command, rest)?;
    let here = std::env::current_exe()
        .map_err(|error| format!("cannot find this program's own path: {error}"))?;
    let change = Suite::at(here.parent().unwrap_or(Path::new(".")))?;
    let base = options.compare.as_deref().map(Suite::at).transpose()?;
    let chosen: Vec<&Benchmark> = BENCHMARKS
        .iter()
        .filter(|benchmark| {
            options.names.is_empty()
                || options.names.iter().any(|name| {
                    match BENCHMARKS.iter().any(|named| named.name == name) {
                        true => benchmark.name == name,
                        false => benchmark.name.contains(name.as_str()),
                    }
                })
        })
        .collect();
    if chosen.is_empty() {
        return Err(format!(
            "no benchmark is named like {}; `bench list` lists them",
            options.names.join(" or ")
        ));
    }
    let logs = std::env::temp_dir().join(format!("bench-{}", std::process::id()));
    std::fs::create_dir_all(&logs)
        .map_err(|error| format!("could not create {}: {error}", logs.display()))?;
    let mut samples = [Samples::default(), Samples::default()];
    for round in 0..options.rounds {
        let mut suites: Vec<(usize, &Suite)> = match &base {
            Some(base) => vec![(0, base), (1, &change)],
            None => vec![(1, &change)],
        };
        if round % 2 == 1 {
            suites.reverse();
        }
        for benchmark in &chosen {
            for (index, suite) in &suites {
                let label = match (base.is_some(), index) {
                    (false, _) => "",
                    (true, 0) => " of BASE",
                    (true, _) => " of this suite",
                };
                eprintln!(
                    "round {}/{}: {}{label}",
                    round + 1,
                    options.rounds,
                    benchmark.name
                );
                let log = logs.join(format!("{}-{index}-{round}.log", benchmark.name));
                let metrics = suite.run(benchmark, &log, options.verbose)?;
                samples[*index].add(benchmark.name, metrics);
            }
        }
    }
    let [before, after] = samples;
    println!("{}", report(base.as_ref().map(|_| &before), &after));
    Ok(())
}

fn parse(command: &str, arguments: &[String]) -> Result<Options, String> {
    let mut options = Options {
        compare: None,
        rounds: 3,
        verbose: false,
        names: Vec::new(),
    };
    let mut rest = arguments.iter();
    if command == "compare" {
        let base = rest.next().ok_or_else(|| USAGE.to_owned())?;
        options.compare = Some(PathBuf::from(base));
    }
    for argument in rest {
        if let Some(rounds) = argument.strip_prefix("--rounds=") {
            options.rounds = rounds
                .parse()
                .ok()
                .filter(|rounds| *rounds > 0)
                .ok_or_else(|| format!("--rounds takes a count, not {rounds}"))?;
        } else if argument == "--verbose" {
            options.verbose = true;
        } else if argument.starts_with('-') {
            return Err(USAGE.to_owned());
        } else {
            options.names.push(argument.clone());
        }
    }
    Ok(options)
}

fn report(base: Option<&Samples>, change: &Samples) -> String {
    let mut rows = vec![match base {
        Some(_) => vec![
            "metric".to_owned(),
            "base fastest".to_owned(),
            "fastest".to_owned(),
            "change".to_owned(),
            "base median".to_owned(),
            "median".to_owned(),
        ],
        None => vec![
            "metric".to_owned(),
            "fastest".to_owned(),
            "median".to_owned(),
            "slowest".to_owned(),
        ],
    }];
    let mut names = change.order.clone();
    if let Some(base) = base {
        names.extend(
            base.order
                .iter()
                .filter(|name| !change.values.contains_key(*name))
                .cloned(),
        );
    }
    for name in names {
        let unit = change
            .units
            .get(&name)
            .or_else(|| base.and_then(|base| base.units.get(&name)))
            .cloned()
            .unwrap_or_default();
        let ours = change.values.get(&name).map(Vec::as_slice).unwrap_or(&[]);
        let shown = |values: &[f64], summary: fn(&[f64]) -> f64| match values.is_empty() {
            true => "-".to_owned(),
            false => number(summary(values), &unit),
        };
        match base {
            Some(base) => {
                let theirs = base.values.get(&name).map(Vec::as_slice).unwrap_or(&[]);
                let difference = match (theirs.is_empty(), ours.is_empty()) {
                    (false, false) => {
                        let (before, after) = (fastest(theirs), fastest(ours));
                        match before == 0.0 {
                            true => "-".to_owned(),
                            false => format!("{:+.1}%", (after - before) / before * 100.0),
                        }
                    }
                    _ => "-".to_owned(),
                };
                rows.push(vec![
                    name,
                    shown(theirs, fastest),
                    shown(ours, fastest),
                    difference,
                    shown(theirs, median),
                    shown(ours, median),
                ]);
            }
            None => rows.push(vec![
                name,
                shown(ours, fastest),
                shown(ours, median),
                shown(ours, |values| {
                    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                }),
            ]),
        }
    }
    table(&rows)
}
