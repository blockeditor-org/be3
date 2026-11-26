use std::cell::RefCell;
use std::rc::Rc;

use be_sdk_type_api::embed::host::Call;
use be_sdk_type_api::guest::host::PartWriter;
use be_sdk_type_api::ops::guest::{self as ops, Executor, Serve};
use be_sdk_type_api::ops::{Connection, Incoming, OpError, Outcome, Side};

use super::*;
use crate::replicas::{Feed, write_part};

/// A module's `part-writer` written a piece at a time: the host seals each write as it comes
/// (`Peer::part_writer`), and `finish` answers the part a whole put of the same bytes makes,
/// held on the server for the block, so another device reads it back whole before any commit
/// names it. The replica's reach keeps it pinned from the vault's collection until a commit
/// names it (`Reach::named`).
#[test]
fn a_part_a_module_writes_in_pieces_is_held_and_read_back_whole() {
    with_another_device(|peer, another, _shared, _replicas| async move {
        let block = block(&peer).await;
        let bytes: Vec<u8> = (0..300_000u32)
            .map(|at| (at.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();

        let module = Executor::new(Box::new(|_, _| {
            Serve::Done(Outcome::Err(OpError::Unsupported))
        }));
        let mut host = Connection::new(Side::Host);
        let finished = Rc::new(RefCell::new(None));
        let writer = module.enter(|| {
            let writer = PartWriter::new();
            for piece in bytes.chunks(70_000) {
                writer.write(piece.to_vec()).detach();
            }
            let finish = writer.finish();
            let finished = Rc::clone(&finished);
            ops::spawn(async move {
                *finished.borrow_mut() = Some(finish.await);
            });
            writer
        });

        let answers = Rc::new(RefCell::new(Vec::new()));
        let (feed, fed) = tokio::sync::mpsc::unbounded_channel();
        let reach: Arc<std::sync::Mutex<crate::replicas::reach::Reach>> = Arc::default();
        tokio::task::spawn_local(write_part(
            Arc::clone(&peer),
            block,
            false,
            Arc::clone(&reach),
            fed,
            Arc::default(),
            {
                let answers = Rc::clone(&answers);
                move |answered| answers.borrow_mut().push(answered)
            },
            |_| panic!("the block is not locked"),
        ));
        let mut writes = 0;
        while finished.borrow().is_none() {
            for answered in answers.borrow_mut().drain(..) {
                answered.send(&mut host);
            }
            let mut outbound = Vec::new();
            module
                .step(host.take_outbound(), &mut outbound)
                .expect("the module steps");
            for frame in outbound {
                host.receive(&frame)
                    .expect("the host takes the module's frame");
            }
            while let Some(event) = host.next_event() {
                let Incoming::Started(started) = event else {
                    continue;
                };
                match Call::accept(&mut host, started) {
                    Some(Call::PartWriterNew { .. }) | None => {}
                    Some(Call::PartWriterWrite { reply, bytes, .. }) => {
                        writes += 1;
                        let _ = feed.send(Feed::Write(bytes, reply));
                    }
                    Some(Call::PartWriterFinish { reply, .. }) => {
                        let _ = feed.send(Feed::Finish(reply));
                    }
                    Some(_) => panic!("the module only writes a part"),
                }
            }
            tokio::task::yield_now().await;
        }
        drop(writer);
        assert_eq!(writes, 5, "each piece is a write of its own");
        let part = finished
            .borrow_mut()
            .take()
            .expect("finished")
            .expect("the host answered")
            .expect("the part is put");
        assert_eq!(part.length(), bytes.len() as u64);

        // Held by the server, named by no commit: the reach's pin keeps it here.
        let objects = peer.part_objects(&[part]).expect("the part is held");
        let store = peer.commits().vault().store();
        assert_eq!(reach.lock().unwrap().pinned(), 1);
        assert_eq!(peer.collect_unnamed().expect("collects"), 0, "pinned");
        for object in &objects {
            assert!(
                be_block_store::ObjectStore::has(store, *object).unwrap(),
                "{object} stays while pinned"
            );
        }
        reach.lock().unwrap().named([part]);
        assert_eq!(reach.lock().unwrap().pinned(), 0);
        assert_eq!(
            peer.collect_unnamed().expect("collects"),
            objects.len(),
            "let go of, the commit that would name it never made, it is collected"
        );

        assert_eq!(
            another
                .read_part(part, 0, part.length())
                .await
                .expect("another device reads the held part"),
            bytes,
            "the part reads back whole"
        );
        assert_eq!(
            put_part(&peer, block, &bytes).await.expect("put whole"),
            part,
            "a part put whole is the part written in pieces"
        );
    });
}
