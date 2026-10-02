use std::sync::{Condvar, Mutex};

pub struct OnceEvent {
    val: Mutex<bool>,
    cond: Condvar,
}

impl OnceEvent {
    pub fn new() -> Self {
        OnceEvent {
            cond: Condvar::new(),
            val: Mutex::new(false),
        }
    }

    pub fn wait(&self) {
        drop(
            self.cond
                .wait_while(self.val.lock().unwrap(), |x| *x == false)
                .unwrap(),
        );
    }

    pub fn trigger(&self) {
        *self.val.lock().unwrap() = true;
        self.cond.notify_all();
    }
}
