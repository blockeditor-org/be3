use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use crate::ui::PerformanceRow;

const SAMPLE_CAPACITY: usize = 120;
const PACING: &str = crate::plugin_host::PACING;

#[derive(Clone)]
enum Value {
    Duration,
    Count(u64),
}

#[derive(Default)]
struct Measurement {
    value: Option<Value>,
    samples: VecDeque<Duration>,
}

#[derive(Default)]
struct Group {
    seen_frame: u64,
    measurements: BTreeMap<String, Measurement>,
}

#[derive(Default)]
struct PerformanceState {
    frame: u64,
    frame_start: Option<Instant>,
    update_start: Option<Instant>,
    update_end: Option<Instant>,
    shown: Option<(Instant, Vec<PerformanceRow>)>,
    frame_times: VecDeque<Duration>,
    groups: BTreeMap<String, Group>,
}

fn state() -> MutexGuard<'static, PerformanceState> {
    static STATE: OnceLock<Mutex<PerformanceState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(PerformanceState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn begin_frame() {
    let mut state = state();
    state.frame = state.frame.wrapping_add(1);
    state.frame_start = Some(Instant::now());
}

pub fn end_frame() {
    let mut state = state();
    let elapsed = state.frame_start.take().map(|start| start.elapsed());
    if let Some(elapsed) = elapsed {
        push_sample(&mut state.frame_times, elapsed);
    }
}

pub fn record_update(started: Instant, document: &beui::PerformanceTimings) {
    const CONTINUOUS: Duration = Duration::from_millis(100);
    let (previous, ended) = {
        let mut state = state();
        let previous = state.update_start.replace(started);
        let ended = state.update_end.replace(Instant::now());
        (previous, ended)
    };
    if let Some(interval) = previous.map(|previous| started - previous)
        && interval < CONTINUOUS
    {
        record_duration_in(PACING, "Frame interval", interval);
        if let Some(ended) = ended {
            record_duration_in(
                PACING,
                "Between frames",
                started.saturating_duration_since(ended),
            );
        }
    }
    record_duration_in(PACING, "Host frame", started.elapsed());
    for (name, duration) in [
        ("Host input", document.interaction),
        ("Host layout", document.layout),
        ("Host paint", document.paint),
        ("Host accessibility", document.accessibility),
    ] {
        record_duration_in(PACING, name, duration);
    }
}

pub fn record_group_duration(group: &str, id: &str, duration: Duration) {
    record_duration_in(group, id, duration);
}

pub fn record_group_count(group: &str, id: &str, count: u64) {
    record_count_in(group, id, count);
}

fn record_duration_in(group: &str, id: &str, duration: Duration) {
    let mut state = state();
    let frame = state.frame;
    let group = state.groups.entry(group.to_owned()).or_default();
    group.seen_frame = frame;
    let measurement = group.measurements.entry(id.to_owned()).or_default();
    measurement.value = Some(Value::Duration);
    push_sample(&mut measurement.samples, duration);
}

fn record_count_in(group: &str, id: &str, count: u64) {
    let mut state = state();
    let frame = state.frame;
    let group = state.groups.entry(group.to_owned()).or_default();
    group.seen_frame = frame;
    group.measurements.entry(id.to_owned()).or_default().value = Some(Value::Count(count));
}

fn push_sample(samples: &mut VecDeque<Duration>, duration: Duration) {
    if samples.len() == SAMPLE_CAPACITY {
        samples.pop_front();
    }
    samples.push_back(duration);
}

pub fn rows() -> Vec<PerformanceRow> {
    const REFRESH: Duration = Duration::from_millis(250);
    let mut state = state();
    if let Some((at, rows)) = &state.shown
        && at.elapsed() < REFRESH
    {
        return rows.clone();
    }
    let rows = build_rows(&state);
    state.shown = Some((Instant::now(), rows.clone()));
    rows
}

fn build_rows(state: &PerformanceState) -> Vec<PerformanceRow> {
    let frame = state.frame;
    let mut rows = vec![
        PerformanceRow {
            heading: false,
            name: "Build".to_owned(),
            current: match cfg!(debug_assertions) {
                true => "dev, unoptimised".to_owned(),
                false => "release".to_owned(),
            },
            average: String::new(),
            peak: String::new(),
        },
        timing_row("Full frame", &state.frame_times),
    ];
    for (id, group) in state
        .groups
        .iter()
        .filter(|(_, group)| group.seen_frame + 1 >= frame)
    {
        rows.push(PerformanceRow {
            heading: true,
            name: id.clone(),
            current: String::new(),
            average: String::new(),
            peak: String::new(),
        });
        for (id, measurement) in &group.measurements {
            match measurement.value {
                Some(Value::Duration) => rows.push(timing_row(id, &measurement.samples)),
                Some(Value::Count(count)) => rows.push(PerformanceRow {
                    heading: false,
                    name: id.clone(),
                    current: count.to_string(),
                    average: "-".to_owned(),
                    peak: "-".to_owned(),
                }),
                None => {}
            }
        }
    }
    rows
}

fn timing_row(id: &str, samples: &VecDeque<Duration>) -> PerformanceRow {
    let current = samples.back().copied().unwrap_or_default();
    let total = samples.iter().copied().sum::<Duration>();
    let average = if samples.is_empty() {
        Duration::ZERO
    } else {
        total / samples.len() as u32
    };
    let peak = samples.iter().copied().max().unwrap_or_default();
    PerformanceRow {
        heading: false,
        name: id.to_owned(),
        current: format_duration(current),
        average: format_duration(average),
        peak: format_duration(peak),
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:7.3} ms", duration.as_secs_f64() * 1_000.0)
}
