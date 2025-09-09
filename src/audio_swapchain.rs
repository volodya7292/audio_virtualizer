use concurrent_queue as cq;

pub struct AudioSwapchain {
    rb: cq::ConcurrentQueue<f32>,
    input_bufs: cq::ConcurrentQueue<Vec<f32>>,
    output_bufs: cq::ConcurrentQueue<Vec<f32>>,
}

impl AudioSwapchain {
    pub fn new(in_buf_size: usize, out_buf_size: usize, n_packets: usize) -> Self {
        let rb_size = in_buf_size.max(out_buf_size) * n_packets;

        let rb = cq::ConcurrentQueue::bounded(rb_size);
        let input_bufs = cq::ConcurrentQueue::bounded(n_packets);
        let output_bufs = cq::ConcurrentQueue::bounded(n_packets);

        for _ in 0..input_bufs.capacity().unwrap() {
            input_bufs.push(vec![0.0; in_buf_size]).unwrap();
            output_bufs.push(vec![0.0; out_buf_size]).unwrap();
        }

        Self {
            rb,
            input_bufs,
            output_bufs,
        }
    }

    pub fn acquire_ready_output_buf(&self) -> Option<Vec<f32>> {
        let mut buf = self.output_bufs.pop().ok()?;

        if self.rb.len() < buf.len() {
            self.output_bufs.push(buf).unwrap();
            return None;
        }

        for bs in &mut buf {
            if let Ok(s) = self.rb.pop() {
                *bs = s;
            } else {
                self.output_bufs.push(buf).unwrap();
                return None;
            }
        }

        Some(buf)
    }

    pub fn acquire_free_input_buf(&self) -> Option<Vec<f32>> {
        self.input_bufs.pop().ok()
    }

    pub fn submit_input_slice(&self, data: &[f32]) {
        for s in data {
            let _ = self.rb.force_push(*s);
        }
    }

    pub fn submit_input_buf(&self, buf: Vec<f32>) {
        self.submit_input_slice(&buf);
        self.input_bufs.push(buf).unwrap();
    }

    pub fn release_output_buf(&self, buf: Vec<f32>) {
        self.output_bufs.push(buf).unwrap();
    }
}
