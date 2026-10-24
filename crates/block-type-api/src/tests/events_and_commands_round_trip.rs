use super::*;

#[test]
fn events_and_commands_round_trip() {
    let head = CommitId::from_hash(be_store::Hash::of(b"head"));
    let events = vec![
        Event::Tick { now: 750 },
        Event::Open {
            replica: 1,
            block: Uuid::from_u128(2),
            content_type: Uuid::from_u128(3),
            organization: Uuid::from_u128(4),
            client: 5,
            tag: SessionTag([6; 32]),
            head: Some(head),
            session: SessionState {
                generation: 1,
                owner: Some(5),
                participants: vec![5],
                clean_at: None,
            },
        },
        Event::Message {
            replica: 1,
            from: Endpoint::Editor(9),
            bytes: vec![1, 2, 3],
        },
        Event::Head {
            replica: 1,
            event: HeadEvent::Rejected {
                token: 3,
                head: Some(head),
            },
        },
        Event::Resumed {
            replica: 1,
            token: 4,
            result: Ok(Resume::Merge {
                base: None,
                ours: head,
                theirs: head,
            }),
        },
        Event::Request {
            id: 7,
            request: Request::Change {
                replica: 1,
                change: Change::Child(ChildChange::Replace {
                    old: Uuid::from_u128(1),
                    new: Uuid::from_u128(2),
                }),
            },
        },
    ];
    for event in events {
        assert_eq!(decode_frame::<Event>(&encode_frame(&event)).unwrap(), event);
    }
    let commands = vec![
        Command::Publish {
            replica: 1,
            token: 2,
            expected: Some(head),
            merging: Some(Merging {
                base: None,
                ours: head,
            }),
            content: Written {
                bytes: vec![4],
                references: vec![Uuid::from_u128(8)],
                derived: Entries::from([(Uuid::from_u128(9), vec![1])]),
            },
        },
        Command::Send {
            replica: 1,
            to: Recipients::Peers,
            bytes: vec![0; 3],
        },
        Command::Reply {
            id: 7,
            reply: Reply::Merged {
                bytes: vec![1],
                conflicts: 2,
            },
        },
    ];
    for command in commands {
        assert_eq!(
            decode_frame::<Command>(&encode_frame(&command)).unwrap(),
            command
        );
    }
}
