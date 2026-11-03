//! Idle diagnostics only: Tokio decides when there is no runnable work.
//! No ready queue, scheduling policy, timer polling or private executor lives here.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    task::Waker,
};
thread_local! {
    static DRIVERS: RefCell<BTreeMap<u64, Weak<Driver>>> = const { RefCell::new(BTreeMap::new()) };
    static SERIAL: Cell<u64> = const { Cell::new(0) };
}
#[derive(Debug)]
pub(crate) struct Driver {
    id: u64,
    group: usize,
    // None is runnable/startup. Some(true) has a real external event source.
    waiting: Cell<Option<bool>>,
    stalled: Cell<bool>,
    wake: RefCell<Option<Waker>>,
}
impl Driver {
    pub fn new(group: usize) -> Rc<Self> {
        let id = SERIAL.with(|serial| {
            let id = serial
                .get()
                .checked_add(1)
                .expect("driver identities exhausted");
            serial.set(id);
            id
        });
        let driver = Rc::new(Self {
            id,
            group,
            waiting: Cell::new(None),
            stalled: Cell::new(false),
            wake: RefCell::new(None),
        });
        DRIVERS.with(|drivers| drivers.borrow_mut().insert(id, Rc::downgrade(&driver)));
        driver
    }
    pub fn running(&self) {
        self.waiting.set(None);
    }
    pub fn pending(&self, external: bool, waker: &Waker) {
        self.waiting.set(Some(external));
        let mut wake = self.wake.borrow_mut();
        if wake.as_ref().is_none_or(|old| !old.will_wake(waker)) {
            *wake = Some(waker.clone());
        }
    }
    pub fn take_stalled(&self) -> bool {
        self.stalled.replace(false)
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        DRIVERS.with(|drivers| drivers.borrow_mut().remove(&self.id));
    }
}

/// Called by Tokio immediately before it parks, after runnable tasks are drained.
pub(crate) fn on_idle() {
    let drivers: Vec<_> = DRIVERS.with(|drivers| {
        drivers
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect()
    });
    let mut groups = BTreeMap::<usize, (bool, Rc<Driver>)>::new();
    for driver in drivers {
        let entry = groups
            .entry(driver.group)
            .or_insert_with(|| (true, driver.clone()));
        entry.0 &= driver.waiting.get() == Some(false);
    }
    for (_, (stalled, driver)) in groups {
        if stalled {
            driver.stalled.set(true);
            if let Some(waker) = driver.wake.borrow().as_ref() {
                waker.wake_by_ref();
            }
        }
    }
}
