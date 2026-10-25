use super::*;

#[test]
fn a_block_is_written_once_to_a_stable_path_and_what_the_program_saved_is_taken_back() {
    let scratch = Scratch::new();
    let block = Uuid::from_u128(7);
    let first = place(&scratch.0, block, &book(b"chapter one")).expect("the book is written");
    assert_eq!(
        first.disk.path,
        scratch.0.join(block.to_string()).join("Moby Dick.epub")
    );
    assert_eq!(std::fs::read(&first.disk.path).unwrap(), b"chapter one");
    assert_eq!(first.taken_back, None);

    let sidecar = first.disk.path.with_file_name("Moby Dick.sdr");
    std::fs::create_dir_all(&sidecar).unwrap();
    let again = place(&scratch.0, block, &book(b"chapter one")).expect("the book is placed again");
    assert_eq!(
        again.disk, first.disk,
        "unchanged bytes are not written again, so the file and what the program keeps beside it stay"
    );
    assert!(sidecar.is_dir());

    let edited = place(&scratch.0, block, &book(b"chapter one, edited elsewhere")).unwrap();
    assert_eq!(
        std::fs::read(&edited.disk.path).unwrap(),
        b"chapter one, edited elsewhere",
        "a block changed elsewhere is written over a file the program left alone"
    );
    assert_eq!(edited.taken_back, None);

    std::fs::write(&edited.disk.path, b"saved by the program, longer").unwrap();
    let (name, bytes, stamp) = changed(&edited.disk).expect("the program's save is seen");
    assert_eq!(name, "Moby Dick.epub");
    assert_eq!(bytes, b"saved by the program, longer");
    assert!(
        changed(&OnDisk {
            path: edited.disk.path.clone(),
            stamp
        })
        .is_none(),
        "a file seen at its stamp has not changed"
    );

    let recovered = place(&scratch.0, block, &book(b"chapter one, edited elsewhere")).unwrap();
    assert_eq!(
        recovered.taken_back,
        Some((
            "Moby Dick.epub".to_owned(),
            b"saved by the program, longer".to_vec()
        )),
        "a save that was not taken back before the program closed is taken back instead of written over"
    );
    assert_eq!(
        std::fs::read(&recovered.disk.path).unwrap(),
        b"saved by the program, longer"
    );
}
