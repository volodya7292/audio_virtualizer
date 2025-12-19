use concurrent_queue as cq;
use ringbuf::traits::{Consumer, Observer, Producer};

pub struct AudioSwapchain {
    input_bufs: cq::ConcurrentQueue<Vec<f32>>,
    output_bufs: cq::ConcurrentQueue<Vec<f32>>,
    desired_rb_size: usize,
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

        let input_bufs =
            cq::ConcurrentQueue::bounded(rb_size.next_multiple_of(in_buf_size) / in_buf_size);
        let output_bufs =
            cq::ConcurrentQueue::bounded(rb_size.next_multiple_of(out_buf_size) / out_buf_size);

        for _ in 0..input_bufs.capacity().unwrap() {
            input_bufs.push(vec![0.0; in_buf_size]).unwrap();
        }
        for _ in 0..output_bufs.capacity().unwrap() {
            output_bufs.push(vec![0.0; out_buf_size]).unwrap();
        }

        Self {
            input_bufs,
            output_bufs,
            desired_rb_size: rb_size,
        }
    }

    pub fn acquire_ready_output_buf(
        &self,
        cons: &mut ringbuf::HeapCons<f32>,
    ) -> Option<AudioBuffer<'_>> {
        let mut buf = AudioBuffer {
            data: self.output_bufs.pop().ok()?,
            free_queue: &self.output_bufs,
        };

        if cons.occupied_len() < buf.data.len() {
            return None;
        }

        let num_returned = cons.pop_slice(buf.data_mut());
        if num_returned < buf.data.len() {
            println!(
                "Warning: expected to read {} samples, but only got {}",
                buf.data.len(),
                num_returned
            );
        }

        Some(buf)
    }

    pub fn acquire_free_input_buf(&self) -> Option<AudioBuffer<'_>> {
        self.input_bufs.pop().ok().map(|buf| AudioBuffer {
            data: buf,
            free_queue: &self.input_bufs,
        })
    }

    pub fn submit_input(data: &[f32], prod: &mut ringbuf::HeapProd<f32>) {
        let num_pushed = prod.push_slice(data);
        if num_pushed < data.len() {
            println!(
                "Warning: expected to push {} samples, but only pushed {}",
                data.len(),
                num_pushed
            );
        }
    }

    pub fn desired_rb_size(&self) -> usize {
        self.desired_rb_size
    }
}
