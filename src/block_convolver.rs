use num_complex::Complex;
use num_traits::Zero;
use realfft::{RealFftPlanner, RealToComplex, ComplexToReal};
use std::{collections::VecDeque, iter, sync::Arc, vec};

pub struct BlockConvolver {
    block_size: usize,
    fft_solver: Arc<dyn RealToComplex<f32>>,
    fft_inv_solver: Arc<dyn ComplexToReal<f32>>,
    hrtf_blocks: Vec<Vec<Complex<f32>>>,
    signal_fft_sliding: VecDeque<Vec<Complex<f32>>>,
    signal_double_block: Vec<f32>,
    accum_tmp: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    output_scratch: Vec<f32>,
}

impl BlockConvolver {
    pub fn new(block_size: usize, hrir: &[f32]) -> Self {
        let window_size = block_size * 2;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_solver = planner.plan_fft_forward(window_size);
        let fft_inv_solver = planner.plan_fft_inverse(window_size);
        let complex_len = window_size / 2 + 1;

        let hrtf_blocks: Vec<_> = hrir
            .chunks(block_size)
            .map(|chunk| {
                let mut chunk_padded: Vec<f32> = chunk
                    .iter()
                    .cloned()
                    .chain(iter::repeat(0_f32).take(window_size - chunk.len()))
                    .collect();

                let mut spectrum = fft_solver.make_output_vec();
                fft_solver.process(&mut chunk_padded, &mut spectrum).unwrap();

                let norm_factor = 1.0 / window_size as f32;
                for v in &mut spectrum {
                    *v *= norm_factor;
                }

                spectrum
            })
            .collect();

        let mut signal_fft_sliding = VecDeque::with_capacity(hrtf_blocks.len());
        let signal_double_block = vec![0.0; block_size * 2];
        let accum_tmp = vec![Complex::<f32>::default(); complex_len];

        for _ in 0..hrtf_blocks.len() {
            signal_fft_sliding.push_back(vec![Complex::<f32>::zero(); complex_len]);
        }

        let scratch = fft_solver.make_scratch_vec();
        let output_scratch = vec![0.0; window_size];

        Self {
            block_size,
            fft_solver,
            fft_inv_solver,
            hrtf_blocks,
            signal_fft_sliding,
            signal_double_block,
            accum_tmp,
            scratch,
            output_scratch,
        }
    }

    pub fn process(&mut self, signal_block: &mut [f32]) {
        assert_eq!(signal_block.len(), self.block_size);

        self.signal_double_block[self.block_size..(self.block_size * 2)]
            .copy_from_slice(signal_block);

        let mut fft_block = self.signal_fft_sliding.pop_front().unwrap();
        self.fft_solver
            .process_with_scratch(&mut self.signal_double_block, &mut fft_block, &mut self.scratch)
            .unwrap();

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
            .process_with_scratch(result_fft, &mut self.output_scratch, &mut self.scratch)
            .unwrap();

        signal_block.copy_from_slice(&self.output_scratch[self.block_size..]);

        self.signal_double_block.copy_within(self.block_size.., 0);
    }
}
