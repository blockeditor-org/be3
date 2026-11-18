use super::*;

fn said(log: &[be_block_content::LogItem]) -> Vec<String> {
    log.iter()
        .map(|item| String::from_utf8_lossy(item.payload.as_deref().unwrap_or_default()).into())
        .collect()
}

/// Two devices' editors append to a block's signed log through their replicas: the server's
/// order is the log's, every replica and editor hears every entry, its own too, and each folds
/// them in that one order into what it describes. An entry served again under its id at a later
/// place is not folded twice: the first place of an id is the one kept.
#[test]
fn two_replicas_append_to_one_log_and_fold_it_in_one_order() {
    let mut sim = Sim::<Logbook>::new(2, logbook_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let mut appended = Vec::new();
    for (device, text) in [(0, "one"), (1, "two"), (0, "three"), (1, "four")] {
        let mirror = sim.editors.get_mut(&(device, 1)).unwrap();
        let (token, message) = mirror.append(text.as_bytes().to_vec());
        sim.send_from_editor(device, 1, message);
        appended.push((device, token));
    }
    sim.settle();
    let mut answers: BTreeMap<usize, Vec<(u64, Result<Option<(u64, i64)>, String>)>> =
        BTreeMap::new();
    for device in 0..2 {
        answers.insert(device, sim.editors.get_mut(&(device, 1)).unwrap().take_appended());
    }
    for (device, token) in appended {
        assert!(
            answers[&device]
                .iter()
                .any(|(answered, result)| *answered == token && matches!(result, Ok(Some(_)))),
            "device {device}'s append was not answered with its place: {answers:?}"
        );
    }
    let order: Vec<String> = sim
        .log
        .iter()
        .map(|item| String::from_utf8_lossy(item.payload.as_deref().unwrap()).into())
        .collect();
    assert_eq!(order.len(), 4);
    let folded = Logbook {
        said: order.clone(),
        ..Logbook::default()
    };
    for device in 0..2 {
        let log: Vec<_> = sim.mirror_log(device, 1);
        assert_eq!(said(&log), order, "device {device}'s editor heard another order");
        assert_eq!(
            sim.devices[device]
                .described
                .as_ref()
                .map(|described| described.derived.clone()),
            Some(be_block_content::derived_metadata(&folded)),
            "device {device} folded the log in another order"
        );
    }

    // The server serves the first entry again under its id, later in the log.
    let mut again = sim.log[0].clone();
    again.seq = sim.log.len() as u64;
    sim.log_item(again);
    sim.settle();
    for device in 0..2 {
        assert_eq!(said(&sim.mirror_log(device, 1)), order);
        assert_eq!(
            sim.devices[device]
                .described
                .as_ref()
                .map(|described| described.derived.clone()),
            Some(be_block_content::derived_metadata(&folded)),
            "an entry served again under its id was folded twice"
        );
    }

    // An editor that attaches later reads the log, from its first entry.
    sim.attach(0, 2, true);
    let (_, read) = sim.editors.get_mut(&(0, 2)).unwrap().read_log(0, 10);
    sim.send_from_editor(0, 2, read);
    sim.settle();
    assert_eq!(said(&sim.mirror_log(0, 2)), order);
}
