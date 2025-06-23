use num_complex::Complex;
use num_traits::Zero;
use rustfft::Fft;
use std::{collections::VecDeque, iter, sync::Arc, vec};

pub struct BlockConvolver {
    block_size: usize,
    fft_solver: Arc<dyn Fft<f32>>,
    fft_inv_solver: Arc<dyn Fft<f32>>,
    hrtf_blocks: Vec<Vec<Complex<f32>>>,
    scratch: Vec<Complex<f32>>,
    signal_fft_sliding: VecDeque<Vec<Complex<f32>>>,
    signal_double_block: Vec<f32>,
    accum_tmp: Vec<Complex<f32>>,
}

impl BlockConvolver {
    pub fn new(block_size: usize, hrir: &[f32]) -> Self {
        let window_size = block_size * 2;
        let fft_solver = rustfft::FftPlanner::<f32>::new().plan_fft_forward(window_size);
        let fft_inv_solver = rustfft::FftPlanner::<f32>::new().plan_fft_inverse(window_size);
        let mut scratch = vec![Complex::<f32>::default(); window_size];

        let hrtf_blocks: Vec<_> = hrir
            .chunks(block_size)
            .map(|chunk| {
                let padded_len = chunk.len().next_power_of_two().max(window_size);

                let mut chunk_padded: Vec<_> = chunk
                    .iter()
                    .cloned()
                    .chain(iter::repeat(0_f32).take(padded_len - chunk.len()))
                    .map(Complex::from)
                    .collect();

                fft_solver.process_with_scratch(&mut chunk_padded, &mut scratch);

                let norm_factor = 1.0 / window_size as f32;
                for v in &mut chunk_padded {
                    *v *= norm_factor;
                }

                chunk_padded
            })
            .collect();

        let mut signal_fft_sliding = VecDeque::with_capacity(hrtf_blocks.len());
        let signal_double_block = vec![0.0; block_size * 2];
        let accum_tmp = vec![Complex::<f32>::default(); window_size];

        for _ in 0..hrtf_blocks.len() {
            signal_fft_sliding.push_back(vec![Complex::<f32>::zero(); window_size]);
        }

        Self {
            block_size,
            fft_solver,
            fft_inv_solver,
            scratch,
            hrtf_blocks,
            signal_fft_sliding,
            signal_double_block,
            accum_tmp,
        }
    }

    pub fn process(&mut self, signal_block: &mut [f32]) {
        assert_eq!(signal_block.len(), self.block_size);

        self.signal_double_block[self.block_size..(self.block_size * 2)]
            .copy_from_slice(signal_block);

        let mut fft_block = self.signal_fft_sliding.pop_front().unwrap();
        for (out, in_val) in fft_block.iter_mut().zip(self.signal_double_block.iter()) {
            *out = Complex::from(*in_val);
        }

        self.fft_solver
            .process_with_scratch(&mut fft_block, &mut self.scratch);

        self.signal_fft_sliding.push_back(fft_block);
        self.accum_tmp.fill(Complex::<f32>::zero());

        let result_fft = self
            .signal_fft_sliding
            .iter()
            .rev()
            .zip(self.hrtf_blocks.iter())
            .fold(&mut self.accum_tmp, |accum, (signal_fft, hrtf)| {
                for (accum, (s, h)) in accum.iter_mut().zip(signal_fft.iter().zip(hrtf)) {
                    *accum += s * h;
                }
                accum
            });

        self.fft_inv_solver
            .process_with_scratch(result_fft, &mut self.scratch);

        let result_signal = &result_fft[self.block_size..];
        for (out, res) in signal_block.iter_mut().zip(result_signal.iter()) {
            *out = res.re;
        }

        self.signal_double_block.copy_within(self.block_size.., 0);
    }
}
