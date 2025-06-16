use crate::block_convolver::BlockConvoler;
use std::io::Cursor;

pub struct SurroundVirtualizerConfig<'a> {
    pub fc_wav: &'a [u8],
    pub bl_wav: &'a [u8],
    pub br_wav: &'a [u8],
    pub fl_wav: &'a [u8],
    pub fr_wav: &'a [u8],
    pub sl_wav: &'a [u8],
    pub sr_wav: &'a [u8],
    pub lfe_wav: &'a [u8],
    pub block_size: usize,
}

struct BinauralConvolver {
    left: BlockConvoler,
    right: BlockConvoler,
    left_buf: Vec<f32>,
    right_buf: Vec<f32>,
}

impl BinauralConvolver {
    pub fn new(block_size: usize, left: Vec<f32>, right: Vec<f32>) -> Self {
        Self {
            left: BlockConvoler::new(block_size, &left),
            right: BlockConvoler::new(block_size, &right),
            left_buf: vec![0.0; block_size],
            right_buf: vec![0.0; block_size],
        }
    }

    pub fn process(&mut self, input_block: &[f32], ch_idx: usize, num_input_channels: usize) {
        let input_ch = input_block.iter().skip(ch_idx).step_by(num_input_channels);

        for (i, v) in input_ch.enumerate() {
            self.left_buf[i] = *v;
        }
        self.right_buf.copy_from_slice(&self.left_buf);

        self.left.process(&mut self.left_buf);
        self.right.process(&mut self.right_buf);
    }
}

pub struct SurroundVirtualizer {
    block_size: usize,
    fc_conv: BinauralConvolver,
    fl_conv: BinauralConvolver,
    fr_conv: BinauralConvolver,
    bl_conv: BinauralConvolver,
    br_conv: BinauralConvolver,
    sl_conv: BinauralConvolver,
    sr_conv: BinauralConvolver,
    lfe_conv: BinauralConvolver,
}

impl SurroundVirtualizer {
    const CENTER_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
    const SIDE_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
    const BACK_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
    const LFE_GAIN: f32 = 0.25;

    pub fn new(config: &SurroundVirtualizerConfig) -> Self {
        let fl = wav_to_binaural_convolver(&config.fl_wav, config.block_size);
        let fr = wav_to_binaural_convolver(&config.fr_wav, config.block_size);
        let fc = wav_to_binaural_convolver(&config.fc_wav, config.block_size);
        let bl = wav_to_binaural_convolver(&config.bl_wav, config.block_size);
        let br = wav_to_binaural_convolver(&config.br_wav, config.block_size);
        let sl = wav_to_binaural_convolver(&config.sl_wav, config.block_size);
        let sr = wav_to_binaural_convolver(&config.sr_wav, config.block_size);
        let lfe = wav_to_binaural_convolver(&config.lfe_wav, config.block_size);

        Self {
            block_size: config.block_size,
            fc_conv: fc,
            fl_conv: fl,
            fr_conv: fr,
            bl_conv: bl,
            br_conv: br,
            sl_conv: sl,
            sr_conv: sr,
            lfe_conv: lfe,
        }
    }

    pub fn process(
        &mut self,
        input_block: &[f32],
        num_input_channels: usize,
        stereo_output: &mut [f32],
    ) {
        assert_eq!(stereo_output.len(), self.block_size * 2);

        self.fl_conv.process(input_block, 0, num_input_channels);
        self.fr_conv.process(input_block, 1, num_input_channels);
        self.fc_conv.process(input_block, 2, num_input_channels);
        self.lfe_conv.process(input_block, 3, num_input_channels);
        self.sl_conv.process(input_block, 4, num_input_channels);
        self.sr_conv.process(input_block, 5, num_input_channels);
        self.bl_conv.process(input_block, 6, num_input_channels);
        self.br_conv.process(input_block, 7, num_input_channels);

        for i in 0..self.block_size {
            stereo_output[i * 2] = self.fl_conv.left_buf[i]
                + self.fr_conv.left_buf[i]
                + Self::CENTER_GAIN * self.fc_conv.left_buf[i]
                + Self::BACK_GAIN * self.bl_conv.left_buf[i]
                + Self::BACK_GAIN * self.br_conv.left_buf[i]
                + Self::SIDE_GAIN * self.sl_conv.left_buf[i]
                + Self::SIDE_GAIN * self.sr_conv.left_buf[i]
                + Self::LFE_GAIN * self.lfe_conv.left_buf[i];
            stereo_output[i * 2 + 1] = self.fl_conv.right_buf[i]
                + self.fr_conv.right_buf[i]
                + Self::CENTER_GAIN * self.fc_conv.right_buf[i]
                + Self::BACK_GAIN * self.bl_conv.right_buf[i]
                + Self::BACK_GAIN * self.br_conv.right_buf[i]
                + Self::SIDE_GAIN * self.sl_conv.right_buf[i]
                + Self::SIDE_GAIN * self.sr_conv.right_buf[i]
                + Self::LFE_GAIN * self.lfe_conv.right_buf[i];
        }
    }
}
pub struct Equalizer {
    left: BlockConvoler,
    right: BlockConvoler,
    buf: Vec<f32>,
}

impl Equalizer {
    pub fn new(block_size: usize, eqir: Vec<f32>) -> Self {
        Self {
            left: BlockConvoler::new(block_size, &eqir),
            right: BlockConvoler::new(block_size, &eqir),
            buf: vec![0.0; block_size],
        }
    }

    pub fn process(&mut self, stereo_signal: &mut [f32]) {
        let left_ch = stereo_signal.iter().step_by(2);
        for (i, v) in left_ch.enumerate() {
            self.buf[i] = *v;
        }
        self.left.process(&mut self.buf);
        for (i, v) in self.buf.iter().enumerate() {
            stereo_signal[i * 2] = *v;
        }

        let right_ch = stereo_signal.iter().skip(1).step_by(2);
        for (i, v) in right_ch.enumerate() {
            self.buf[i] = *v;
        }
        self.right.process(&mut self.buf);
        for (i, v) in self.buf.iter().enumerate() {
            stereo_signal[i * 2 + 1] = *v;
        }
    }
}

pub fn wav_to_pcm(wav_data: &[u8]) -> Vec<f32> {
    let mut reader = hound::WavReader::new(Cursor::new(wav_data)).unwrap();
    let pcm = reader
        .samples::<f32>()
        .map(|s| s.unwrap_or_default())
        .collect::<Vec<f32>>();
    pcm
}

fn wav_to_binaural_convolver(wav_data: &[u8], block_size: usize) -> BinauralConvolver {
    let pcm = wav_to_pcm(wav_data);
    let left_pcm = pcm.iter().step_by(2).cloned().collect::<Vec<_>>();
    let right_pcm = pcm.iter().skip(1).step_by(2).cloned().collect::<Vec<_>>();
    BinauralConvolver::new(block_size, left_pcm, right_pcm)
}
