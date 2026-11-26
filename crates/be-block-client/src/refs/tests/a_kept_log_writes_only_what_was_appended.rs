use be_block_keys::{
    Authority, Checked, Grant, Level, LogKind, Record, RecoveryPhrase, Step, check_log,
};
use uuid::Uuid;

use super::*;

/// What `KeyBook::note_log` keeps of a whole log checked as `checked` from `steps`.
fn kept(checked: &Checked, steps: &[Step]) -> KeptLog {
    let place = steps.len() - 1;
    let mut levels: Vec<([u8; 16], Level)> = checked
        .levels
        .iter()
        .filter_map(|(account, changes)| Some((*account, changes.last()?.1)))
        .collect();
    levels.sort();
    KeptLog {
        head: checked.head,
        place: place as u64,
        lowering_place: checked.last_lowering as u64,
        current: steps[checked.last_lowering..]
            .iter()
            .map(Step::hash)
            .collect(),
        levels,
        vouched: Vec::new(),
        checked: Some(checked.clone()),
        steps: steps.to_vec(),
    }
}

/// A device keeps the access log it checked with its refs, as it grows: each append writes the
/// entry it added and the log's own row, not the whole log again, so a long log's appends write
/// about as much each as the first did. Read back after a reopen, the log is as it was kept, and
/// so is a log kept anew in its place, whole or a proof path's.
#[test]
fn a_kept_log_writes_only_what_was_appended() {
    const LENGTH: usize = 400;
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let block = Uuid::new_v4();
    let creator = RecoveryPhrase::generate().secret();
    let (creator_id, log) = ([1; 16], block.into_bytes());
    let root = be_block_keys::Entry::root(&creator, creator_id, LogKind::Block, log);
    let mut steps = vec![Step::from(root.clone())];
    let mut checked = check_log(LogKind::Block, log, root.hash(), &steps, &|_| None).unwrap();
    let grantees: Vec<([u8; 16], _)> = (0..4u8)
        .map(|at| ([at + 2; 16], RecoveryPhrase::generate().secret().identity()))
        .collect();

    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    let mut writes = Vec::new();
    for index in 1..LENGTH {
        let (grantee, identity) = grantees[index % grantees.len()];
        let level = match (index / grantees.len()) % 2 {
            0 => Level::View,
            _ => Level::Edit,
        };
        let entry = checked
            .next(
                &creator,
                creator_id,
                log,
                Record::Grant(Grant {
                    grantee,
                    identity,
                    level,
                    authority: Authority::Log,
                }),
            )
            .unwrap();
        let step = Step::from(entry);
        checked = checked
            .extend(LogKind::Block, log, std::slice::from_ref(&step), &|_| None)
            .unwrap();
        steps.push(step);
        let before = refs.written();
        refs.keep_log(block, kept(&checked, &steps)).unwrap();
        writes.push(refs.written() - before);
    }
    let whole: u64 = encode(&kept(&checked, &steps)).len() as u64;
    let early = writes[10];
    let last = *writes.last().unwrap();
    assert!(
        last <= early * 2,
        "an append to a long log writes about what one to a short log does: {early} then {last}"
    );
    assert!(
        last * 20 < whole,
        "an append writes far less than the whole log: {last} of {whole}"
    );

    let grown = kept(&checked, &steps);
    drop(refs);
    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    assert_eq!(
        refs.kept_logs(),
        vec![(block, grown.clone())],
        "the log reads back as kept"
    );

    // Served as a proof path now, the log is kept anew from its last lowering alone, and the
    // whole log's places no longer kept are dropped.
    let lowering = grown.lowering_place as usize;
    let path_kept = KeptLog {
        checked: None,
        steps: Vec::new(),
        ..grown.clone()
    };
    assert_eq!(path_kept.current.len(), steps.len() - lowering);
    refs.keep_log(block, path_kept.clone()).unwrap();
    drop(refs);
    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    assert_eq!(
        refs.kept_logs(),
        vec![(block, path_kept)],
        "a proof path's kept log reads back as kept"
    );

    // A shorter whole log kept in its place (the server served another) drops the places past it.
    let short = check_log(LogKind::Block, log, root.hash(), &steps[..3], &|_| None).unwrap();
    let short = kept(&short, &steps[..3]);
    refs.keep_log(block, short.clone()).unwrap();
    drop(refs);
    let refs = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert_eq!(
        refs.kept_logs(),
        vec![(block, short)],
        "a log kept in another's place reads back as kept"
    );
    drop(refs);
    let _ = std::fs::remove_file(&path);
}
