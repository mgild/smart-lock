use smart_lock::smart_lock;
use std::rc::Rc;

#[smart_lock]
struct Bad {
    data: Rc<u32>,
}

fn main() {}
