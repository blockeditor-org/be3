use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

const LAZY: Uuid = Uuid::from_u128(3);

static STARTS: AtomicUsize = AtomicUsize::new(0);

struct Counted(Answering);

impl Guest for Counted {
    fn start(_: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        STARTS.fetch_add(1, Ordering::SeqCst);
        Ok(Self(Answering::new(LAZY)))
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.0.receive(inbound);
        outbound.extend(self.0.connection.take_outbound());
        Ok(Flow::Running)
    }
}

#[test]
fn a_declared_module_starts_only_when_its_type_is_first_needed() {
    let mut modules = Modules::new(
        vec![in_process::<Counted>("counted").declaring(vec![LAZY])],
        be_client::Clock::real(),
    );
    pollster::block_on(modules.ready());
    assert_eq!(STARTS.load(Ordering::SeqCst), 0);
    assert!(
        is_known(LAZY),
        "the declared type is known before its module runs"
    );
    assert_eq!(modules.module_of(LAZY), Some(0));
    assert!(kind_of(LAZY).is_none());

    pollster::block_on(modules.load(0));
    assert_eq!(STARTS.load(Ordering::SeqCst), 1);
    assert!(
        kind_of(LAZY).is_some(),
        "loading registers what the module answered"
    );

    pollster::block_on(modules.load(0));
    assert_eq!(STARTS.load(Ordering::SeqCst), 1);
}
