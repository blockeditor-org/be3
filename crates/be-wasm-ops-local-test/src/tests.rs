use be_wasm_ops::local::{Promise, Sender, Settled, Settling};

use super::*;

mod a_call_too_wide_to_pass_flat_is_encoded;
mod a_dropped_ticket_no_longer_waits;
mod a_fallible_call_answers_its_error_only_when_it_fails;
mod a_refused_ticket_stops_the_guest;
mod a_resource_of_another_package_is_one_handle;
mod a_settled_value_that_does_not_decode_stops_the_guest;
mod a_ticket_dropped_before_it_settles_is_named_to_the_host_once;
mod async_funcs_futures_and_streams_settle_through_the_poll;
mod options_of_lists_and_strings_pass_flat;

/// The guest's memory, and the host it calls, as a wasm host would hold them.
struct Caller {
    memory: Vec<u8>,
    host: Host,
    returned: Vec<u8>,
    tickets: u32,
    refused: Vec<String>,
}

impl Caller {
    fn new() -> Self {
        Self {
            memory: vec![0; 4096],
            host: Host::default(),
            returned: Vec::new(),
            tickets: 0,
            refused: Vec::new(),
        }
    }

    /// Puts `bytes` in memory at `pointer` and answers where they end.
    fn put(&mut self, pointer: u32, bytes: &[u8]) -> u32 {
        let start = pointer as usize;
        self.memory[start..start + bytes.len()].copy_from_slice(bytes);
        pointer + bytes.len() as u32
    }

    fn bytes(&self, pointer: u32, length: u32) -> &[u8] {
        &self.memory[pointer as usize..(pointer + length) as usize]
    }

    /// What settled, polled as the generated `settle` polls: naming the tickets `dropped`, put
    /// in memory at 3072.
    fn settled(&mut self, dropped: &[u32]) -> Vec<Settled> {
        let length = match dropped {
            [] => 0,
            dropped => {
                let bytes =
                    be_wasm_ops::wire::encode::<be_wasm_ops::wire::Plain, _>(&dropped.to_vec());
                self.put(3072, &bytes) - 3072
            }
        };
        let length = embed::shapes::glue::settle(self, 0, 3072, length, 1024, 2048);
        be_wasm_ops::decode(self.bytes(1024, length)).expect("what settled decodes")
    }
}

fn read(memory: &[u8], pointer: u32, length: u32) -> Result<Vec<u8>, String> {
    memory
        .get(pointer as usize..(pointer as usize).saturating_add(length as usize))
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "out of bounds".to_owned())
}

#[derive(Default)]
struct Host {
    named: Vec<(Option<String>, Option<Vec<u32>>, Option<Vec<u8>>)>,
    shown: Vec<u32>,
    wide: Vec<(Vec<u32>, u32)>,
    ready: Vec<Promise<()>>,
    grown: Vec<(u32, Promise<u32>)>,
    lost: Vec<Promise<String>>,
    errors: Vec<Sender<shapes::shapes::Failed>>,
    settled: Vec<Settled>,
    dropped: Vec<u32>,
}

impl embed::shapes::Host for Host {
    fn shape_area(&mut self, this: shapes::shapes::Shape) -> u32 {
        this.0
    }

    fn shape_grow(&mut self, this: shapes::shapes::Shape, by: u32, settle: Promise<u32>) {
        self.grown.push((this.0 + by, settle));
    }

    fn shape_label(&mut self, _this: shapes::shapes::Shape) -> Option<String> {
        None
    }

    fn shape_drop(&mut self, _this: shapes::shapes::Shape) {}

    fn make(&mut self, sides: u32) -> shapes::shapes::Shape {
        shapes::shapes::Shape(sides)
    }

    fn name(&mut self, label: Option<&str>, offsets: Option<&[u32]>, bytes: Option<&[u8]>) -> u32 {
        self.named.push((
            label.map(str::to_owned),
            offsets.map(<[u32]>::to_vec),
            bytes.map(<[u8]>::to_vec),
        ));
        self.named.len() as u32
    }

    fn check(&mut self, value: u32) -> Result<(), shapes::shapes::Broken> {
        match value {
            0 => Err(shapes::shapes::Broken::Failed(shapes::shapes::Failed {
                message: "nothing to check".to_owned(),
            })),
            _ => Ok(()),
        }
    }

    fn ready(&mut self, settle: Promise<()>) {
        self.ready.push(settle);
    }

    fn lost(&mut self, settle: Promise<String>) {
        self.lost.push(settle);
    }

    fn errors(&mut self, settle: Sender<shapes::shapes::Failed>) {
        self.errors.push(settle);
    }

    fn settle(&mut self, _wait: bool, dropped: &[u32]) -> Vec<Settled> {
        self.dropped.extend_from_slice(dropped);
        std::mem::take(&mut self.settled)
    }
}

impl embed::frames::Host for Host {
    fn show(&mut self, shape: shapes::shapes::Shape) -> u32 {
        self.shown.push(shape.0);
        shape.0
    }

    fn framed(
        &mut self,
        framing: frames::Framing,
        extra: (u32, u32, u32, u32, u32, u32, u32, u32),
    ) -> u32 {
        self.shown.push(framing.shape.0);
        framing.sizes.8 + extra.7
    }

    fn wide(
        &mut self,
        first: (u32, u32, u32, u32, u32, u32, u32, u32, u32, u32),
        second: (u32, u32, u32, u32, u32, u32, u32, u32),
        last: u32,
    ) -> u32 {
        let (a, b, c, d, e, f, g, h, i, j) = first;
        let (k, l, m, n, o, p, q, r) = second;
        self.wide.push((
            vec![a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r],
            last,
        ));
        last
    }
}

macro_rules! caller {
    ($($interface:ident $(with $ticket:ident)?),*) => {$(
        impl embed::$interface::Caller for Caller {
            type Host = Host;

            fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
                read(&self.memory, pointer, length)
            }

            fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
                let start = pointer as usize;
                let room = self
                    .memory
                    .get_mut(start..start + bytes.len())
                    .ok_or_else(|| "out of bounds".to_owned())?;
                room.copy_from_slice(bytes);
                Ok(())
            }

            fn host<R>(&mut self, _function: &'static str, call: impl FnOnce(&mut Host) -> R) -> Option<R> {
                Some(call(&mut self.host))
            }

            fn refuse(&mut self, message: String) {
                self.refused.push(message);
            }

            fn returned(&mut self) -> &mut Vec<u8> {
                &mut self.returned
            }

            $(fn $ticket(&mut self) -> u32 {
                self.tickets += 1;
                self.tickets
            })?
        }
    )*};
}

caller!(frames, shapes with ticket);

/// Hands what the host settled to a guest's tickets, as the generated `settle` does.
fn settle(caller: &mut Caller, settling: &Settling) {
    let settled = caller.settled(&settling.take_dropped());
    settling.settle(settled);
}
