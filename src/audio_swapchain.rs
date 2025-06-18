use concurrent_queue as cq;

pub struct AudioSwapchain {
    free_bufs: cq::ConcurrentQueue<Vec<f32>>,
    ready_bufs: cq::ConcurrentQueue<Vec<f32>>,
}

impl AudioSwapchain {
    pub fn new(buf_size: usize, n_packets: usize) -> Self {
        let free_bufs = cq::ConcurrentQueue::bounded(n_packets);
        let ready_bufs = cq::ConcurrentQueue::bounded(n_packets);

        for _ in 0..free_bufs.capacity().unwrap() {
            free_bufs.push(vec![0.0; buf_size]).unwrap();
        }

        Self {
            free_bufs,
            ready_bufs,
        }
    }

    pub fn acquire_ready_buf(&self) -> Option<Vec<f32>> {
        self.ready_bufs.pop().ok()
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
    }

    pub fn release_buf(&self, buf: Vec<f32>) {
        self.free_bufs.push(buf).unwrap();
    }
}
