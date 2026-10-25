use super::*;
use crate::plugin_host::wire::Wire;
use block_plugin_api::{
    BlockFilter, BlockPick,
    ops::{Connection, Incoming, OpError, OpId, Outcome, Side},
};

fn carry(from: Vec<Vec<u8>>, to: &mut Connection) -> Vec<Incoming> {
    for frame in from {
        to.receive(&frame).expect("the frame decodes");
    }
    std::iter::from_fn(|| to.next_event()).collect()
}

fn to_host(guest: &mut Connection, wire: &mut Wire) {
    let mut heard = Vec::new();
    for frame in guest.take_outbound() {
        wire.receive(&frame, &mut heard)
            .expect("the plugin's frame decodes");
    }
}

fn opened(wire: &mut Wire, guest: &mut Connection) -> OpId {
    wire.open(Opening::Block(OpenBlock {
        instance: INSTANCE,
        editor: [1; 16],
        block_id: [2; 16],
        block_type: [3; 16],
        view_block: None,
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        editable: true,
    }));
    match carry(wire.take_outbound(), guest).as_slice() {
        [Incoming::Started(started)] => started.op,
        heard => panic!("the plugin heard {heard:?} rather than its editor opening"),
    }
}

fn pick(guest: &mut Connection, editor: OpId) -> OpId {
    let args = block_plugin_api::__args::HostServicesPickBlockArgs {
        filter: BlockFilter::default(),
    };
    guest.start(
        Some(editor),
        block_plugin_api::__args::HOST_SERVICES_PICK_BLOCK,
        block_plugin_api::ops::encode(&args),
    )
}

#[test]
fn closing_an_editor_cancels_the_pick_it_waits_on() {
    let mut instances = placed();
    let mut wire = Wire::default();
    let mut guest = Connection::new(Side::Guest);
    let editor = opened(&mut wire, &mut guest);

    let shown = pick(&mut guest, editor);
    let asked = pick(&mut guest, editor);
    to_host(&mut guest, &mut wire);
    let services = wire.take_services();
    assert!(services.withdrawn.is_empty());
    for service in services.asked {
        assert_eq!(service.instance, INSTANCE, "the picks are the editor's");
        instances.serve(service);
    }
    assert_eq!(
        instances.take_block_pick(INSTANCE).map(|pick| pick.op),
        Some(shown),
        "the first pick is shown, the second waits"
    );

    wire.close(INSTANCE);
    let heard = carry(wire.take_outbound(), &mut guest);
    assert!(
        matches!(heard.as_slice(), [Incoming::Canceled { op }] if *op == editor),
        "closing the editor cancels it: {heard:?}"
    );
    guest.complete(editor, Outcome::returned(&()));
    to_host(&mut guest, &mut wire);

    let services = wire.take_services();
    let mut withdrawn = services.withdrawn.clone();
    withdrawn.sort_unstable();
    assert_eq!(withdrawn, vec![shown, asked]);
    for op in services.withdrawn {
        instances.withdraw(op);
    }
    assert!(
        instances.take_block_pick(INSTANCE).is_none(),
        "the host drops the pick nobody waits for"
    );
    let heard = carry(wire.take_outbound(), &mut guest);
    let mut canceled: Vec<OpId> = heard
        .iter()
        .filter_map(|event| match event {
            Incoming::Completed {
                op,
                outcome: Outcome::Err(OpError::Canceled),
            } => Some(*op),
            _ => None,
        })
        .collect();
    canceled.sort_unstable();
    assert_eq!(
        canceled,
        vec![shown, asked],
        "both picks are answered canceled"
    );

    instances.block_picked(
        INSTANCE,
        shown,
        BlockPick::Chosen {
            block_id: [7; 16],
            block_type: [8; 16],
            linked: false,
            placed: false,
        },
    );
    assert!(
        instances.take_answers().is_empty(),
        "a pick answered after its editor closed reaches nobody"
    );
}
