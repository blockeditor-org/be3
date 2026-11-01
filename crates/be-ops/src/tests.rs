use super::*;

mod a_frame_cut_short_is_refused;
mod a_guest_stream_grants_credit_as_it_takes_items;
mod a_guest_task_awaits_an_import_and_dropping_it_cancels;
mod a_latest_value_stream_sends_only_the_newest_value_once_it_has_room;
mod a_network_call_returns_its_typed_result_from_another_thread;
mod a_network_resource_is_canceled_when_dropped;
mod a_network_stream_grants_credit_as_it_is_read;
mod a_stream_holds_back_once_a_window_of_bytes_is_in_flight;
mod a_stream_runs_no_further_ahead_than_its_consumer_grants;
mod an_op_is_canceled_when_its_parent_ends;
mod bytes_cross_whole_in_every_position;
mod closing_a_network_resource_waits_for_its_end;
mod dropping_a_network_call_cancels_it;
mod every_op_completes_once_even_after_a_cancel;

fn deliver(from: &mut Connection, to: &mut Connection) {
    for frame in from.take_outbound() {
        to.receive(&frame).unwrap();
    }
}

fn events(connection: &mut Connection) -> Vec<Incoming> {
    std::iter::from_fn(|| connection.next_event()).collect()
}

type Queue = std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>>;

struct Pipe {
    client: net::Peer,
    server: net::Peer,
    to_client: Queue,
    to_server: Queue,
}

fn pipe() -> Pipe {
    let (to_client, to_server) = (Queue::default(), Queue::default());
    let sink = |queue: &Queue| {
        let queue = std::sync::Arc::clone(queue);
        move |frame| queue.lock().unwrap().push_back(frame)
    };
    Pipe {
        client: net::Peer::new(Side::Guest, sink(&to_server)),
        server: net::Peer::new(Side::Host, sink(&to_client)),
        to_client,
        to_server,
    }
}

impl Pipe {
    fn pump(&self) -> Vec<Incoming> {
        let mut events = Vec::new();
        loop {
            let to_server = self.to_server.lock().unwrap().pop_front();
            if let Some(frame) = to_server {
                events.extend(self.server.receive(&frame).unwrap());
                continue;
            }
            let to_client = self.to_client.lock().unwrap().pop_front();
            match to_client {
                Some(frame) => assert_eq!(self.client.receive(&frame).unwrap(), vec![]),
                None => return events,
            }
        }
    }

    fn accept_one(&self) -> embed::shelf::Call {
        let calls: [embed::shelf::Call; 1] = self.accept().try_into().unwrap();
        let [call] = calls;
        call
    }

    fn accept(&self) -> Vec<embed::shelf::Call> {
        self.pump()
            .into_iter()
            .filter_map(|event| match event {
                Incoming::Started(started) => {
                    embed::shelf::Call::accept(&mut self.server.lock(), started)
                }
                Incoming::Canceled { .. } => None,
                other => panic!("{other:?} arrived where a call was expected"),
            })
            .collect()
    }
}

fn poll<F: std::future::Future + Unpin>(future: &mut F) -> std::task::Poll<F::Output> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    std::pin::Pin::new(future).poll(&mut context)
}

fn book(pages: u32) -> shelf::Book {
    shelf::Book {
        title: format!("Book {pages}"),
        pages,
    }
}
