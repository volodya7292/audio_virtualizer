use concurrent_queue as cq;
use std::{
    sync::{Condvar, Mutex},
    time::Duration,
};

pub struct AudioBufferQueue {
    free_bufs: cq::ConcurrentQueue<Vec<f32>>,
    ready_bufs: cq::ConcurrentQueue<Vec<f32>>,
    ready_cond_var: (Mutex<()>, Condvar),
}

impl AudioBufferQueue {
    pub fn new(buf_size: usize) -> Self {
        let free_bufs = cq::ConcurrentQueue::bounded(3);
        let ready_bufs = cq::ConcurrentQueue::bounded(1);

        for _ in 0..free_bufs.capacity().unwrap() {
            free_bufs.push(vec![0.0; buf_size]).unwrap();
        }

        Self {
            free_bufs,
            ready_bufs,
            ready_cond_var: (Mutex::new(()), Condvar::new()),
        }
    }

    pub fn acquire_ready_buf(&self, timeout: Duration) -> Option<Vec<f32>> {
        let mut guard = self.ready_cond_var.0.lock().ok()?;

        loop {
            if let Ok(buf) = self.ready_bufs.pop() {
                return Some(buf);
            }
            let res = self.ready_cond_var.1.wait_timeout(guard, timeout).ok()?;
            if res.1.timed_out() {
                return None;
            }
            guard = res.0;
        }
    }

    pub fn acquire_free_buf(&self) -> Option<Vec<f32>> {
        self.free_bufs.pop().ok()
    }

    pub fn submit_buf(&self, buf: Vec<f32>) {
        let Ok(old_buf) = self.ready_bufs.force_push(buf) else {
            return;
        };
        if let Some(old_buf) = old_buf {
            self.free_bufs.push(old_buf).unwrap();
        }
        self.ready_cond_var.1.notify_one();
    }

    pub fn release_buf(&self, buf: Vec<f32>) {
        self.free_bufs.push(buf).unwrap();
    }
}
