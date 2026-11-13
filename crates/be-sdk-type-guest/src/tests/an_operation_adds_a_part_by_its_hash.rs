use super::*;

use be_block_content::{ContentError, Merge, Part, PartRef};
use be_block_commit::MergeResult;
use serde::{Deserialize, Serialize};

/// Segments added by an operation that names each by the part it was put as, with the hash of
/// its bytes and what to rebuild them from, as chat closes a segment. Its first part is the
/// number of segments; each segment is a part of its own, kept as it was put. Its refs come
/// from the parts it loaded (`from_stored`), never from its own bytes: a save after a rotation
/// reseals a reused part under a new ref.
#[derive(Clone, Debug, Default)]
struct Shelf {
    segments: Vec<Segment>,
}

#[derive(Clone, Debug)]
struct Segment {
    /// None only in an editor's copy, decoded from the first part alone.
    part: Option<PartRef>,
    /// The bytes this replica rebuilt, kept only when they hash to what the operation said.
    held: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum ShelfOp {
    Add {
        part: PartRef,
        hash: [u8; 32],
        recipe: Vec<u8>,
    },
}

/// How this build encodes a segment.
fn encoded(recipe: &[u8]) -> Vec<u8> {
    recipe.repeat(4096)
}

/// How another build encodes the same segment.
fn encoded_elsewhere(recipe: &[u8]) -> Vec<u8> {
    let mut bytes = encoded(recipe);
    bytes.reverse();
    bytes
}

impl BlockContent for Shelf {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7368_656c_662d_6669_7874_7572_6500_0001);
    const LOADS: be_block_content::Loads = be_block_content::Loads::FirstPart;

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(&(self.segments.len() as u32)).unwrap()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let count: u32 =
            postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("shelf"))?;
        let unknown = Segment {
            part: None,
            held: None,
        };
        Ok(Self {
            segments: vec![unknown; count as usize],
        })
    }

    fn from_stored(stored: &be_block_content::Stored) -> Result<Self, ContentError> {
        let mut shelf = Self::decode(&stored.bytes)?;
        let saved = stored
            .root
            .as_ref()
            .map(|root| {
                root.children
                    .iter()
                    .skip(1)
                    .map(|part| part.reference)
                    .collect()
            })
            .unwrap_or_else(Vec::new);
        if saved.len() != shelf.segments.len() {
            return Err(ContentError::Malformed("shelf parts"));
        }
        for (segment, part) in shelf.segments.iter_mut().zip(saved) {
            segment.part = Some(part);
        }
        Ok(shelf)
    }

    fn parts(&self) -> Part<'_> {
        let mut parts = vec![Part::from(self.encode())];
        parts.extend(
            self.segments
                .iter()
                .filter_map(|segment| segment.part.map(Part::Reused)),
        );
        Part::node(parts)
    }

    fn reused_bytes(&self, part: PartRef) -> Option<std::borrow::Cow<'_, [u8]>> {
        let segment = self
            .segments
            .iter()
            .find(|segment| segment.part == Some(part))?;
        segment.held.as_deref().map(std::borrow::Cow::Borrowed)
    }
}

impl LiveEdit for Shelf {
    type Op = ShelfOp;

    fn apply(&mut self, operation: &ShelfOp) {
        let ShelfOp::Add { part, hash, recipe } = operation;
        let rebuilt = encoded(recipe);
        let held = (*Hash::of(&rebuilt).as_bytes() == *hash).then_some(rebuilt);
        self.segments.push(Segment {
            part: Some(*part),
            held,
        });
    }
}

impl Merge for Shelf {
    fn merge3(_: &Self, ours: &Self, _: &Self) -> MergeResult<Self> {
        MergeResult::Clean(ours.clone())
    }
}

fn shelf_module() -> Module {
    Module::new(vec![Registration::of::<Shelf>()])
}

/// Puts `bytes` from device 1's editor, as a sender does before it operates.
fn put(sim: &mut Sim<Shelf>, bytes: Vec<u8>) -> PartRef {
    let (token, message) = sim.editors.get_mut(&(1, 1)).unwrap().put_part(bytes);
    sim.send_from_editor(1, 1, message);
    sim.settle();
    let put = sim.editors.get_mut(&(1, 1)).unwrap().take_parts_put();
    match put.as_slice() {
        [(answered, Ok(part))] if *answered == token => *part,
        other => panic!("the part was not put: {other:?}"),
    }
}

fn add(sim: &mut Sim<Shelf>, recipe: &[u8], bytes: Vec<u8>) -> (PartRef, Vec<u8>) {
    let hash = *Hash::of(&bytes).as_bytes();
    let part = put(sim, bytes.clone());
    sim.operate(
        1,
        1,
        ShelfOp::Add {
            part,
            hash,
            recipe: recipe.to_vec(),
        },
    );
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    (part, bytes)
}

fn stored(sim: &mut Sim<Shelf>, device: usize) -> Vec<u8> {
    let ticket = sim.with_replica(device, |replica, host| replica.content(host));
    let outcome = sim.answered(device, ticket.op());
    ticket
        .decode(outcome)
        .expect("content is answered")
        .expect("the replica is readable")
}

#[test]
fn an_operation_adds_a_part_by_its_hash() {
    let mut sim = Sim::<Shelf>::new(2, shelf_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);

    let (first, first_bytes) = add(&mut sim, b"twelve bytes", encoded(b"twelve bytes"));
    assert_eq!(sim.parts_put, 1);
    let head = sim.head.expect("the owner committed");
    let outline = sim
        .vault
        .outline(&sim.commits[&head].manifest)
        .unwrap()
        .expect("the head has parts");
    assert_eq!(
        outline.children.last().map(|part| part.reference),
        Some(first),
        "the owner's commit reuses the part the follower put"
    );
    assert!(
        sim.largest_relay < 1024 && sim.largest_written < 1024,
        "the segment was relayed or written again: {} and {} bytes",
        sim.largest_relay,
        sim.largest_written
    );
    let mut expected = Shelf {
        segments: vec![Segment {
            part: Some(first),
            held: None,
        }],
    }
    .encode();
    expected.extend_from_slice(&first_bytes);
    assert_eq!(stored(&mut sim, 0), expected[..]);
    assert_eq!(
        sim.part_reads, 0,
        "a replica that rebuilt the bytes reads nothing"
    );

    let (second, second_bytes) = add(
        &mut sim,
        b"from elsewhere",
        encoded_elsewhere(b"from elsewhere"),
    );
    let mut expected = Shelf {
        segments: vec![
            Segment {
                part: Some(first),
                held: None,
            },
            Segment {
                part: Some(second),
                held: None,
            },
        ],
    }
    .encode();
    expected.extend_from_slice(&first_bytes);
    expected.extend_from_slice(&second_bytes);
    for device in [0, 1] {
        assert_eq!(stored(&mut sim, device), expected[..]);
    }
    assert_eq!(
        sim.part_reads, 2,
        "a replica that rebuilt other bytes read the part instead"
    );
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
