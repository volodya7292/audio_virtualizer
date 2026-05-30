use crate::audio_data::AFrame;
use concurrent_queue as cq;
use ringbuf::traits::{Consumer, Observer, Producer};

pub struct AudioSwapchain<const NUM_CHANNELS: usize> {
    bufs: cq::ConcurrentQueue<Vec<f32>>,
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

impl<const NUM_CHANNELS: usize> AudioSwapchain<NUM_CHANNELS> {
    pub fn new(pool_buf_size: usize, peer_buf_size: usize, min_num_packets: usize) -> Self {
        let rb_size = pool_buf_size.max(peer_buf_size) * min_num_packets;

        let bufs =
            cq::ConcurrentQueue::bounded(rb_size.next_multiple_of(pool_buf_size) / pool_buf_size);
        for _ in 0..bufs.capacity().unwrap() {
            bufs.push(vec![0.0; pool_buf_size]).unwrap();
        }

        Self {
            bufs,
            desired_rb_size: rb_size,
        }
    }

    pub fn acquire_ready_output_buf(
        &self,
        cons: &mut ringbuf::HeapCons<AFrame<NUM_CHANNELS>>,
    ) -> Option<AudioBuffer<'_>> {
        let mut buf = AudioBuffer {
            data: self.bufs.pop().ok()?,
            free_queue: &self.bufs,
        };

        let num_frames = buf.data.len() / NUM_CHANNELS;
        if cons.occupied_len() < num_frames {
            return None;
        }

        for (idx, frame) in cons.pop_iter().enumerate().take(num_frames) {
            for ch in 0..NUM_CHANNELS {
                buf.data[idx * NUM_CHANNELS + ch] = frame[ch];
            }
        }

        Some(buf)
    }

    pub fn acquire_free_input_buf(&self) -> Option<AudioBuffer<'_>> {
        self.bufs.pop().ok().map(|buf| AudioBuffer {
            data: buf,
            free_queue: &self.bufs,
        })
    }

    /// Submits input audio data into the ring buffer producer.
    /// Returns the number of frames successfully pushed.
    pub fn submit_input(data: &[f32], prod: &mut ringbuf::HeapProd<AFrame<NUM_CHANNELS>>) -> usize {
        let num_frames = data.len() / NUM_CHANNELS;

        let num_pushed_frames = prod.push_iter((0..num_frames).map(|idx| {
            let mut frame = [0.0; NUM_CHANNELS];
            for ch in 0..NUM_CHANNELS {
                frame[ch] = data[idx * NUM_CHANNELS + ch];
            }
            frame
        }));

        num_pushed_frames
    }

    /// Drains `output.len() / NUM_CHANNELS` frames from the ring buffer consumer into
    /// the interleaved `output` slice. Returns `false` if fewer frames are available.
    pub fn drain_output(
        cons: &mut ringbuf::HeapCons<AFrame<NUM_CHANNELS>>,
        output: &mut [f32],
    ) -> bool {
        let num_frames = output.len() / NUM_CHANNELS;
        if cons.occupied_len() < num_frames {
            return false;
        }

        for (idx, frame) in cons.pop_iter().enumerate().take(num_frames) {
            for ch in 0..NUM_CHANNELS {
                output[idx * NUM_CHANNELS + ch] = frame[ch];
            }
        }

        true
    }

    pub fn desired_rb_size(&self) -> usize {
        self.desired_rb_size
    }
}
