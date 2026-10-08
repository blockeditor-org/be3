use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use text_editor_core::{Document, Position};

use super::{Caret, Side, SideDocument, Simulation};

mod carets_arrive_after_the_edits_they_point_into;
mod edits_wait_for_the_latency_before_they_arrive;
mod paused_typing_on_both_sides_converges_once_sent_both_ways;

fn simulated(text: &str) -> (Arc<RwLock<Simulation>>, SideDocument, SideDocument) {
    let simulation = Arc::new(RwLock::new(Simulation::new(text)));
    let left = SideDocument::new(&simulation, Side::Left);
    let right = SideDocument::new(&simulation, Side::Right);
    (simulation, left, right)
}

fn typed(document: &SideDocument, at: usize, text: &str) {
    document.edit(Vec::new(), &mut |edit| edit.replace(at, 0, text.as_bytes()));
    document.finish_history_group();
}

fn text(simulation: &Arc<RwLock<Simulation>>, side: Side) -> String {
    simulation.read().unwrap().text(side)
}

fn send(simulation: &Arc<RwLock<Simulation>>, from: Side) {
    simulation
        .write()
        .unwrap()
        .deliver(from, Instant::now(), None);
}
