use super::*;
use crate::frozen_example::FrozenPoint;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Duration;
use uuid::Uuid;

fn kind<T>(result: Result<T, Error>) -> ErrorKind {
    match result {
        Ok(_) => panic!("decoded"),
        Err(error) => error.kind,
    }
}

fn bytes<T: Encode + ?Sized>(value: &T) -> Vec<u8> {
    encode(value).expect("encodes")
}

/// An older build's view of a person.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct PersonV1 {
    name: String,
    age: u32,
}

/// A newer build's: two fields more.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct PersonV2 {
    name: String,
    age: u32,
    email: Option<String>,
    tags: Vec<String>,
}

/// The older build again, keeping what it does not know.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct PersonKeeping {
    name: String,
    age: u32,
    #[schema(unknown)]
    rest: Unknown,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
enum ShapeV1 {
    #[schema(default)]
    Dot,
    Circle(f64),
    #[schema(unknown)]
    Other(UnknownVariant),
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
enum ShapeV2 {
    #[schema(default)]
    Dot,
    Circle(f64),
    Square {
        side: f64,
        label: String,
    },
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
enum ClosedShape {
    Dot,
    Circle(f64),
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Holder {
    shape: ClosedShape,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct HolderOfHolder {
    holder: Holder,
}

/// An older build's setting.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct SettingV1 {
    name: String,
}

/// A newer build's, with a field older builds must not read past.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct SettingV2 {
    name: String,
    #[schema(critical)]
    unit: u32,
}

/// Another newer build's, with a field older builds may read past but must not write back.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct SettingV3 {
    name: String,
    #[schema(no_copy)]
    checksum: u32,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct SettingKeeping {
    name: String,
    #[schema(unknown)]
    rest: Unknown,
}

/// A setting an older build holds without reading it.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct KeepsSetting {
    setting: Kept<SettingV1>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct HasSetting {
    setting: SettingV2,
}

/// An older build's holder, reading the setting in place.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct HasOldSetting {
    setting: SettingV1,
}

/// A type that knows no fields, keeping all of them.
#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct KeepsAll {
    #[schema(unknown)]
    rest: Unknown,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Narrow {
    count: u32,
    ratio: f32,
    offsets: Vec<i16>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Wide {
    count: u64,
    ratio: f64,
    offsets: Vec<i64>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
enum ModeV1 {
    Off,
    #[schema(unknown)]
    Other(UnknownVariant),
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
enum ModeV2 {
    Off,
    On,
    #[schema(critical)]
    Strict(u8),
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Drawing {
    shape: ShapeV2,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Wrapper {
    inner: PersonV1,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Config {
    #[schema(default = true)]
    enabled: bool,
    #[schema(default = 7)]
    retries: u32,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Blob {
    data: Vec<u8>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Borrowed<'a> {
    name: &'a str,
    data: &'a [u8],
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Opt {
    count: Option<u32>,
    nested: Option<Option<u32>>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Id(u64);

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Nothing;

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Pair<T> {
    first: T,
    second: T,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Tree {
    value: i32,
    children: Vec<Tree>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Linked {
    to: Reference,
    many: Vec<Reference>,
    child: Option<Box<Linked>>,
}

#[derive(Debug, PartialEq, Schema)]
#[schema(unfrozen)]
struct Everything {
    flag: bool,
    small: u8,
    negative: i64,
    huge: u128,
    letter: char,
    ratio: f32,
    id: Id,
    key: [u8; 4],
    chunks: Vec<Vec<u8>>,
    names: BTreeMap<String, Vec<u8>>,
    counts: HashMap<String, u32>,
    seen: BTreeSet<u16>,
    tags: HashSet<String>,
    pair: (u8, String),
    triple: (bool, i16, Option<u8>),
    pairs: Pair<String>,
    shape: ShapeV2,
    boxed: Box<PersonV1>,
    shared: std::sync::Arc<String>,
    nothing: Nothing,
    unit: (),
    nonzero: std::num::NonZeroU32,
    waited: Duration,
    uuid: Uuid,
    letters: Vec<char>,
    flags: Vec<bool>,
    wide: Vec<u128>,
}

fn person_v2() -> PersonV2 {
    PersonV2 {
        name: "Ada".into(),
        age: 36,
        email: Some("ada@example.com".into()),
        tags: vec!["maths".into(), "engines".into()],
    }
}

fn chain(depth: usize) -> Tree {
    let mut tree = Tree {
        value: 0,
        children: Vec::new(),
    };
    for value in 1..depth {
        tree = Tree {
            value: value as i32,
            children: vec![tree],
        };
    }
    tree
}

/// A deterministic stream of bytes (xorshift), so the test is the same every run.
fn noise(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

mod a_canonical_decode_refuses_any_other_bytes;
mod a_closed_enum_refuses_variants_it_does_not_know;
mod a_field_takes_a_default_of_its_own;
mod a_kept_value_fails_only_whoever_asks_for_it;
mod a_missing_field_decodes_as_its_default;
mod a_missing_field_with_no_default_is_an_error_naming_where;
mod a_no_copy_field_is_read_past_but_never_written_back;
mod a_number_widens_but_never_narrows;
mod a_reader_keeps_unknown_fields_and_writes_them_back_in_place;
mod a_reader_refuses_what_it_must_understand_and_does_not;
mod a_reader_skips_fields_it_does_not_know;
mod a_unit_variant_and_an_empty_message_are_a_key_alone;
mod an_open_enum_keeps_variants_it_does_not_know;
mod block_references_are_found_without_the_schema;
mod borrowed_fields_point_into_the_input;
mod bytes_travel_as_one_run;
mod decoding_allocates_within_a_budget;
mod deep_nesting_is_refused_both_ways_not_a_stack_overflow;
mod every_kind_of_value_reads_back;
mod fuzzed_inputs_never_panic_and_built_values_read_back;
mod malformed_input_is_refused;
mod numbers_are_fixed_width_and_their_lists_are_their_memory;
mod some_of_a_default_is_not_none;
mod the_length_is_known_before_writing;
mod the_lockfile_numbers_a_frozen_type;
mod the_same_value_is_always_the_same_bytes;
