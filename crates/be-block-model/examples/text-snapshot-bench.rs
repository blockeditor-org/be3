//! Times what an editor does with a snapshot of a 1 MB text: decoding the stored form and
//! adopting the replica's session state. `./scripts/buck run -m release
//! //crates/be-block-model:example-text-snapshot-bench`.

use std::time::{Duration, Instant};

use be_block_model::{Document, Model, ObjectId, Text};

const BYTES: usize = 1 << 20;
const ROUNDS: usize = 100;
const CLIENT: u64 = 1;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Note {
    body: Text,
}

fn prose() -> Vec<u8> {
    let mut seed = 0x5eed_u64;
    let mut text = Vec::with_capacity(BYTES);
    while text.len() < BYTES {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let word = 2 + (seed % 9) as usize;
        text.extend((0..word).map(|at| b'a' + ((seed >> (at * 5)) % 26) as u8));
        text.push(match seed % 13 {
            0 => b'\n',
            _ => b' ',
        });
    }
    text.truncate(BYTES);
    text
}

fn main() {
    let mut document = Document::new(&Note {
        body: Text::new(prose()),
    });
    for at in [10, BYTES / 3, BYTES / 2, BYTES - 10] {
        let body = document
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body");
        let typed = body
            .insert(CLIENT, at, b"typed".to_vec())
            .expect("there is somewhere to type");
        document.apply(&Note::BODY.edit(ObjectId::ROOT, typed).into());
    }
    let bytes = document.to_bytes();
    let session = document.session_state();

    let mut taken = Vec::with_capacity(ROUNDS);
    for _ in 0..ROUNDS {
        let start = Instant::now();
        let mut loaded = Document::<Note>::from_bytes(&bytes).expect("the snapshot decodes");
        loaded
            .adopt_session_state(&session)
            .expect("the session state applies");
        taken.push(start.elapsed());
        assert!(loaded == document, "the snapshot loads back what was saved");
    }
    taken.sort();
    let fastest = taken[0];
    let median = taken[ROUNDS / 2];
    println!(
        "decoding and adopting a {} KiB text's snapshot: fastest {:.3} ms, median {:.3} ms",
        BYTES / 1024,
        ms(fastest),
        ms(median)
    );
    be_tool_bench::time("text-snapshot.decode-and-adopt", median);
}

fn ms(taken: Duration) -> f64 {
    taken.as_secs_f64() * 1000.0
}
