use super::*;

mod a_minute_keeps_the_peak_of_each_measurement_through_it;
mod memory_is_read_from_proc_status_in_bytes;
mod the_file_keeps_two_days_of_minutes;
mod the_performance_panel_graphs_memory;
mod two_days_of_minutes_are_kept;

/// Sizes in MiB.
fn usage(resident: u64, wasm: u64) -> MemoryUsage {
    MemoryUsage {
        resident: resident << 20,
        wasm: wasm << 20,
        ..MemoryUsage::default()
    }
}

fn sample(at: u64, resident: u64, guest: u64) -> Sample {
    Sample {
        at,
        usage: usage(resident, guest),
        guests: vec![("app be3:desktop".to_owned(), guest << 20)],
    }
}
