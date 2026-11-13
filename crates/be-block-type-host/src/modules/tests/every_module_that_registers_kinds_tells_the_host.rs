use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

const EAGER: Uuid = Uuid::from_u128(4);
const ASKED: Uuid = Uuid::from_u128(5);

static TOLD: AtomicUsize = AtomicUsize::new(0);

fn told() {
    TOLD.fetch_add(1, Ordering::SeqCst);
}

macro_rules! answering {
    ($name:ident, $content_type:expr) => {
        struct $name(Answering);

        impl Guest for $name {
            fn start(_: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
                Ok(Self(Answering::new($content_type)))
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
    };
}

answering!(Eager, EAGER);
answering!(Asked, ASKED);

/// A module launched with the modules says hello on its own, and one declared starts when
/// asked: each tells the host of the kinds it registers (`Host::kinds_changed`), however it
/// started.
#[test]
fn every_module_that_registers_kinds_tells_the_host() {
    be_lib_build_id::install("test");
    let mut modules = Modules::with_kinds_changed(
        vec![
            in_process::<Eager>("eager"),
            in_process::<Asked>("asked").declaring(vec![ASKED]),
        ],
        be_block_client::Clock::real(),
        told,
    );
    pollster::block_on(modules.ready());
    assert!(kind_of(EAGER).is_some());
    assert_eq!(
        TOLD.load(Ordering::SeqCst),
        1,
        "the eager module's hello told the host"
    );

    pollster::block_on(modules.load(1));
    assert!(kind_of(ASKED).is_some());
    assert_eq!(
        TOLD.load(Ordering::SeqCst),
        2,
        "the declared module, started when asked, told the host"
    );
}
