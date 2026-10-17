use std::collections::HashMap;

use uuid::Uuid;

use crate::transfer::swap_ids;

fn swapped_slowly(bytes: &[u8], ids: &HashMap<Uuid, Uuid>) -> Vec<u8> {
    let mut forms = Vec::new();
    for (old, new) in ids {
        forms.push((
            old.hyphenated().to_string().into_bytes(),
            new.hyphenated().to_string().into_bytes(),
        ));
        forms.push((
            old.simple().to_string().into_bytes(),
            new.simple().to_string().into_bytes(),
        ));
        forms.push((old.as_bytes().to_vec(), new.as_bytes().to_vec()));
    }
    let mut out = Vec::new();
    let mut at = 0;
    'scan: while at < bytes.len() {
        for (old, new) in &forms {
            if bytes[at..].starts_with(old) {
                out.extend(new);
                at += old.len();
                continue 'scan;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    out
}

#[test]
fn swapping_ids_replaces_every_written_form_of_each() {
    let ids: HashMap<Uuid, Uuid> = (0..50).map(|_| (Uuid::now_v7(), Uuid::now_v7())).collect();
    let olds: Vec<Uuid> = ids.keys().copied().collect();
    let unrelated = Uuid::now_v7();
    let mut bytes = olds[0].as_bytes().to_vec();
    for (index, old) in olds.iter().enumerate() {
        bytes.extend(format!("x{index}\"").as_bytes());
        bytes.extend(old.hyphenated().to_string().as_bytes());
        bytes.extend(old.simple().to_string().as_bytes());
        bytes.extend(old.as_bytes());
        bytes.extend(unrelated.hyphenated().to_string().as_bytes());
        bytes.extend(&old.as_bytes()[..12]);
    }
    bytes.extend(olds[1].simple().to_string().as_bytes());

    let swapped = swap_ids(&bytes, &ids);
    assert_eq!(swapped, swapped_slowly(&bytes, &ids));
    for old in &olds {
        assert!(
            !swapped
                .windows(36)
                .any(|window| window == old.hyphenated().to_string().as_bytes()),
            "no old id is left"
        );
    }
    assert_eq!(swap_ids(b"short", &ids), b"short");
}
