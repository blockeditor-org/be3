use super::*;

#[test]
fn chunking_is_content_defined() {
    let data = pseudorandom(64 * 1024, 7);
    let first = split(&data, ChunkerConfig::SMALL);
    assert_eq!(first, split(&data, ChunkerConfig::SMALL));
    assert!(first.len() > 8, "expected many chunks, got {}", first.len());
    assert!(first.iter().all(|range| range.len() <= 1024));
    assert_eq!(first.last().unwrap().end, data.len());

    let mut edited = data.clone();
    edited.splice(0..0, *b"prefix");
    let second = split(&edited, ChunkerConfig::SMALL);

    let unchanged = first
        .iter()
        .filter(|range| {
            second
                .iter()
                .any(|other| edited[other.clone()] == data[(*range).clone()])
        })
        .count();
    assert!(
        unchanged * 4 > first.len() * 3,
        "a six byte prefix resynchronised only {unchanged} of {} chunks",
        first.len()
    );
}
