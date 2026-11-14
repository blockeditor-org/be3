use std::sync::{Arc, mpsc};
use std::thread;

use super::{Mutex, initialize_page, initialize_worker, may_wait};

mod a_page_may_not_wait_and_takes_a_free_lock_at_once;
mod a_thread_off_the_page_waits_for_a_held_lock;
