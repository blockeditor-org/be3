use super::*;

mod a_frame_cut_short_is_refused;
mod a_guest_stream_grants_credit_as_it_takes_items;
mod a_guest_task_awaits_an_import_and_dropping_it_cancels;
mod a_latest_value_stream_sends_only_the_newest_value_once_it_has_room;
mod a_stream_holds_back_once_a_window_of_bytes_is_in_flight;
mod a_stream_runs_no_further_ahead_than_its_consumer_grants;
mod an_op_is_canceled_when_its_parent_ends;
mod every_op_completes_once_even_after_a_cancel;

fn deliver(from: &mut Connection, to: &mut Connection) {
    for frame in from.take_outbound() {
        to.receive(&frame).unwrap();
    }
}

fn events(connection: &mut Connection) -> Vec<Incoming> {
    std::iter::from_fn(|| connection.next_event()).collect()
}
