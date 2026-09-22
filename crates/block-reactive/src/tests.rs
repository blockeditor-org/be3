use std::cell::Cell;
use std::rc::Rc;

use block_client::BlockClient;
use block_client::block_ref::BlockRef;
use block_client::blocks::presentation::{Presentation, PresentationOperation, PresentationSlide};
use reactive::{Scope, create_effect};
use uuid::Uuid;

use crate::BlockSource;

mod an_idle_pump_does_not_run_its_projections_again;
mod editing_one_item_wakes_only_the_bindings_that_read_it;
mod operating_through_the_source_is_visible_before_it_returns;

fn client() -> BlockClient {
    BlockClient::new(Uuid::new_v4(), Uuid::new_v4())
}

fn add_slide(index: usize) -> PresentationOperation {
    PresentationOperation::Insert {
        slide: PresentationSlide {
            id: Uuid::new_v4(),
            block_id: BlockRef::Direct(Uuid::new_v4()),
        },
        index,
    }
}
