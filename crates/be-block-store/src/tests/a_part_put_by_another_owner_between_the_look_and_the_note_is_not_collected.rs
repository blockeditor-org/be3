use std::{collections::HashSet, sync::atomic::AtomicBool};

use super::*;

/// One owner's part writer finds a chunk missing, and before it notes the chunk another owner's
/// vault over the same store puts the same bytes whole, which claims nothing since nothing is
/// noted yet. The writer looks again once it noted the chunk and forgets it, so no collection
/// of its owner's removes what the other owner's write names.
#[test]
fn a_part_put_by_another_owner_between_the_look_and_the_note_is_not_collected() {
    let store = MemoryStore::new();
    let unnamed = Unnamed::default();
    let interrupting = Interrupting {
        inner: store.clone(),
        during: Arc::default(),
    };
    let first = Vault::new(interrupting.clone(), keys(1))
        .with_chunker(ChunkerConfig::SMALL)
        .with_unnamed(unnamed.clone(), Hash::of(b"first"));
    let second = Arc::new(
        Vault::new(store.clone(), keys(1))
            .with_chunker(ChunkerConfig::SMALL)
            .with_unnamed(unnamed, Hash::of(b"second")),
    );
    let bytes = Arc::new(pseudorandom(8 * 1024, 9));

    let done = Arc::new(AtomicBool::new(false));
    let manifest = Arc::new(Mutex::new(None));
    *interrupting.during.lock().unwrap() = Some(Box::new({
        let second = Arc::clone(&second);
        let bytes = Arc::clone(&bytes);
        let manifest = Arc::clone(&manifest);
        move || {
            if !done.swap(true, Ordering::Relaxed) {
                let written = second.write(id(1), CONTENT, bytes.as_slice()).unwrap();
                *manifest.lock().unwrap() = Some(written);
            }
        }
    }));
    let mut writer = first.part_writer(id(1)).unwrap();
    writer.write(&bytes).unwrap();
    let written = writer.finish().unwrap();
    *interrupting.during.lock().unwrap() = None;
    let manifest = manifest.lock().unwrap().take().expect("put meanwhile");
    assert_eq!(written.part(), manifest.root, "the same part");
    drop(written);

    assert_eq!(first.collect(|_| Ok(HashSet::new())).unwrap(), 0);
    for object in second.objects(&manifest).unwrap() {
        assert!(store.has(object).unwrap(), "{object} stays");
    }
}
