use stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

thread_local! {
    static IMPORTED: RefCell<HashMap<u64, Weak<wgpu::Texture>>> = RefCell::new(HashMap::default());
}

pub fn preview_texture(image: u64) -> Option<Rc<wgpu::Texture>> {
    IMPORTED.with_borrow_mut(|imported| {
        if let Some(texture) = imported.get(&image).and_then(Weak::upgrade) {
            return Some(texture);
        }
        imported.retain(|_, held| held.strong_count() > 0);
        let texture = Rc::new(block_gpu_guest::import_texture(image)?);
        imported.insert(image, Rc::downgrade(&texture));
        Some(texture)
    })
}
