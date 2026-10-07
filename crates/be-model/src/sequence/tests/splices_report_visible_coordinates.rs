use super::*;

#[test]
fn splices_report_visible_coordinates() {
    let mut sequence = loaded("hello");
    let insert = sequence
        .insert(ALICE, 5, b" world".to_vec())
        .expect("there is something to insert");
    assert_eq!(
        applied(&mut sequence, &insert),
        [Splice {
            at: 5,
            removed: 0,
            inserted: 6
        }]
    );

    let delete = sequence.delete(0..6).expect("there is something to delete");
    assert_eq!(
        applied(&mut sequence, &delete),
        [
            Splice {
                at: 0,
                removed: 5,
                inserted: 0
            },
            Splice {
                at: 0,
                removed: 1,
                inserted: 0
            }
        ]
    );

    let moved = sequence
        .move_range(0..2, 5)
        .expect("the range moves somewhere else");
    assert_eq!(
        applied(&mut sequence, &moved),
        [
            Splice {
                at: 0,
                removed: 2,
                inserted: 0
            },
            Splice {
                at: 3,
                removed: 0,
                inserted: 2
            }
        ]
    );
    assert_eq!(text(&sequence), "rldwo");
}
