use concurrent_queue as cq;

pub struct AudioSwapchain {
    rb: cq::ConcurrentQueue<f32>,
    input_bufs: cq::ConcurrentQueue<Vec<f32>>,
    output_bufs: cq::ConcurrentQueue<Vec<f32>>,
}

pub struct AudioBuffer<'a> {
    data: Vec<f32>,
    free_queue: &'a cq::ConcurrentQueue<Vec<f32>>,
}

impl AudioBuffer<'_> {
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }
}

impl Drop for AudioBuffer<'_> {
    fn drop(&mut self) {
        self.free_queue
            .push(std::mem::take(&mut self.data))
            .unwrap();
    }
}

impl AudioSwapchain {
    pub fn new(in_buf_size: usize, out_buf_size: usize, min_num_packets: usize) -> Self {
        let rb_size = in_buf_size.max(out_buf_size) * min_num_packets;

        let rb = cq::ConcurrentQueue::bounded(rb_size);
        let input_bufs =
            cq::ConcurrentQueue::bounded(rb_size.next_multiple_of(in_buf_size) / in_buf_size);
        let output_bufs =
            cq::ConcurrentQueue::bounded(rb_size.next_multiple_of(out_buf_size) / out_buf_size);

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

    pub fn acquire_ready_output_buf(&self) -> Option<AudioBuffer<'_>> {
        let mut buf = AudioBuffer {
            data: self.output_bufs.pop().ok()?,
            free_queue: &self.output_bufs,
        };

        if self.rb.len() < buf.data.len() {
            return None;
        }

        for bs in &mut buf.data {
            if let Ok(s) = self.rb.pop() {
                *bs = s;
            } else {
                return None;
            }
        }

        Some(buf)
    }

    pub fn acquire_free_input_buf(&self) -> Option<AudioBuffer<'_>> {
        self.input_bufs.pop().ok().map(|buf| AudioBuffer {
            data: buf,
            free_queue: &self.input_bufs,
        })
    }

    pub fn submit_input(&self, data: &[f32]) {
        for s in data {
            let _ = self.rb.force_push(*s);
        }
    }
}
