use super::*;
use crate::frozen_example::FrozenPoint;
use std::collections::BTreeMap;

/// An older build's view of a person.
#[derive(Debug, PartialEq, Schema)]
struct PersonV1 {
    name: String,
    age: u32,
}

/// A newer build's: two fields more.
#[derive(Debug, PartialEq, Schema)]
struct PersonV2 {
    name: String,
    age: u32,
    email: Option<String>,
    tags: Vec<String>,
}

/// The older build again, keeping what it does not know.
#[derive(Debug, PartialEq, Schema)]
struct PersonKeeping {
    name: String,
    age: u32,
    #[schema(unknown)]
    rest: Unknown,
}

#[derive(Debug, PartialEq, Schema)]
enum ShapeV1 {
    #[schema(default)]
    Dot,
    Circle(f64),
    #[schema(unknown)]
    Other(UnknownVariant),
}

#[derive(Debug, PartialEq, Schema)]
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
enum ClosedShape {
    Dot,
    Circle(f64),
}

#[derive(Debug, PartialEq, Schema)]
struct Holder {
    shape: ClosedShape,
}

#[derive(Debug, PartialEq, Schema)]
struct Drawing {
    shape: ShapeV2,
}

#[derive(Debug, PartialEq, Schema)]
struct Blob {
    data: Vec<u8>,
}

#[derive(Debug, PartialEq, Schema)]
struct Borrowed<'a> {
    name: &'a str,
    data: &'a [u8],
}

#[derive(Debug, PartialEq, Schema)]
struct Opt {
    count: Option<u32>,
    nested: Option<Option<u32>>,
}

#[derive(Debug, PartialEq, Schema)]
struct Id(u64);

#[derive(Debug, PartialEq, Schema)]
struct Nothing;

#[derive(Debug, PartialEq, Schema)]
struct Tree {
    value: i32,
    children: Vec<Tree>,
}

#[derive(Debug, PartialEq, Schema)]
struct Everything {
    flag: bool,
    small: u8,
    negative: i64,
    letter: char,
    ratio: f32,
    id: Id,
    key: [u8; 4],
    chunks: Vec<Vec<u8>>,
    names: BTreeMap<String, Vec<u8>>,
    counts: HashMap<String, u32>,
    shape: ShapeV2,
    boxed: Box<PersonV1>,
    nothing: Nothing,
}

fn person_v2() -> PersonV2 {
    PersonV2 {
        name: "Ada".into(),
        age: 36,
        email: Some("ada@example.com".into()),
        tags: vec!["maths".into(), "engines".into()],
    }
}

mod a_closed_enum_refuses_variants_it_does_not_know;
mod a_default_variant_is_left_out_as_a_field;
mod a_missing_field_decodes_as_its_default;
mod a_missing_field_with_no_default_is_an_error;
mod a_reader_keeps_unknown_fields_and_writes_them_back_in_place;
mod a_reader_skips_fields_it_does_not_know;
mod an_open_enum_keeps_variants_it_does_not_know;
mod borrowed_fields_point_into_the_input;
mod bytes_travel_as_one_run;
mod deep_nesting_is_refused_not_a_stack_overflow;
mod every_kind_of_value_reads_back;
mod fuzzed_inputs_never_panic_and_built_values_read_back;
mod malformed_input_is_refused;
mod numbers_are_fixed_width_and_their_lists_are_their_memory;
mod some_of_a_default_is_not_none;
mod the_lockfile_numbers_a_frozen_type;
mod the_same_value_is_always_the_same_bytes;
