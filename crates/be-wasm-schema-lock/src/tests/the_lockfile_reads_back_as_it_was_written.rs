use super::*;

#[test]
fn the_lockfile_reads_back_as_it_was_written() {
    let source = "be_wasm_schema::frozen!();
        mod inner {
            #[derive(Debug, be_wasm_schema::Schema)]
            enum Shape { #[schema(default)] Dot, Circle(f64), Square { side: f64 }, #[schema(unknown)] Other(Unknown) }
        }
        #[derive(Schema)] struct Id(u64);
        #[derive(Schema)] struct Pairs(Vec<[u8; 4]>);
        #[derive(Schema)] struct Unit;
        #[derive(Schema)] struct Page<'a> { title: &'a str, tags: BTreeMap<String, Vec<u8>>, #[schema(unknown)] rest: Unknown }";
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let lock = lock_of(&[("a.rs", source)], &old).unwrap();
    let text = lock.to_text();
    assert_eq!(Lock::parse(&text).unwrap(), lock);
    assert_eq!(
        text,
        "\
# Written by ./scripts/verify from this crate's Schema types (be-wasm-schema): the id of each
# field and variant. Rename a member of a frozen type here when you rename it in the source.

newtype Id: u64
  frozen from 1

struct Page
  frozen from 3
  1 title: &'a str
  2 tags: BTreeMap<String, Vec<u8>>

newtype Pairs: Vec<[u8; 4]>
  frozen from 1

enum Shape
  open
  frozen from 4
  1 Dot
  2 Circle(f64)
  3 Square{}

struct Shape::Square
  frozen from 2
  1 side: f64

struct Unit
  frozen from 1
"
    );
}
