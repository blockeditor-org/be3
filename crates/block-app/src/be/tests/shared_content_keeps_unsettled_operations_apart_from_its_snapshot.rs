use super::*;

use be_block::Landed;

fn add(by: i64) -> Vec<u8> {
    CounterContent::encode_operation(&Counter::add(by))
}

fn snapshot(content: &Content) -> (i64, Vec<Vec<u8>>) {
    let Update::Snapshot {
        bytes, unsettled, ..
    } = content.since(TEST_ORIGIN, None)
    else {
        panic!("an editor with nothing is sent a snapshot");
    };
    let settled = CounterContent::decode(&bytes)
        .expect("the counter decodes")
        .root()
        .value();
    (settled, unsettled)
}

fn shown(content: &Content) -> i64 {
    CounterContent::decode(content.bytes())
        .expect("the counter decodes")
        .root()
        .value()
}

#[test]
fn shared_content_keeps_unsettled_operations_apart_from_its_snapshot() {
    let mut content = Content::new(
        CounterContent::CONTENT_TYPE,
        CounterContent::default().encode(),
        Vec::new(),
        None,
    );
    for (by, unsettled) in [(2, 1), (3, 2)] {
        content.record(worker::Logged::Operation {
            landed: Landed::On {
                operation: add(by),
                unsettled,
            },
            origin: Some(TEST_ORIGIN),
        });
    }
    assert_eq!(shown(&content), 5);
    assert_eq!(snapshot(&content), (0, vec![add(2), add(3)]));

    let beneath = worker::Logged::Operation {
        landed: Landed::Beneath {
            operation: add(10),
            unsettled: 2,
            local: vec![add(2), add(3)],
        },
        origin: None,
    };
    assert!(content.lands(std::slice::from_ref(&beneath)));
    content.record(beneath);
    assert_eq!(shown(&content), 15);
    assert_eq!(snapshot(&content), (10, vec![add(2), add(3)]));

    content.compact();
    assert_eq!(content.unfolded(), 0);
    assert_eq!(snapshot(&content), (10, vec![add(2), add(3)]));
    assert!(
        !content.lands(&[worker::Logged::Operation {
            landed: Landed::Beneath {
                operation: add(1),
                unsettled: 3,
                local: Vec::new(),
            },
            origin: None,
        }]),
        "an operation cannot go beneath more unsettled ones than the content holds"
    );
}
