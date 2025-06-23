use crate::audio_data::{AudioDataMut, AudioDataRef};
use crate::block_convolver::BlockConvolver;
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
    left: BlockConvolver,
    right: BlockConvolver,
    left_out: Vec<f32>,
    right_out: Vec<f32>,
}

impl BinauralConvolver {
    pub fn new(block_size: usize, left: Vec<f32>, right: Vec<f32>) -> Self {
        Self {
            left: BlockConvolver::new(block_size, &left),
            right: BlockConvolver::new(block_size, &right),
            left_out: vec![0.0; block_size],
            right_out: vec![0.0; block_size],
        }
    }

    pub fn process<'a>(&mut self, input_ch_block: impl Iterator<Item = &'a f32>) {
        for (i, v) in input_ch_block.enumerate() {
            self.left_out[i] = *v;
        }
        self.right_out.copy_from_slice(&self.left_out);

        self.left.process(&mut self.left_out);
        self.right.process(&mut self.right_out);
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

    pub fn process(&mut self, input_block: &AudioDataRef, stereo_output: &mut AudioDataMut) {
        assert_eq!(stereo_output.data.len(), self.block_size * 2);

        self.fl_conv.process(input_block.select_channel(0));
        self.fr_conv.process(input_block.select_channel(1));
        self.fc_conv.process(input_block.select_channel(2));
        self.lfe_conv.process(input_block.select_channel(3));
        self.sl_conv.process(input_block.select_channel(4));
        self.sr_conv.process(input_block.select_channel(5));
        self.bl_conv.process(input_block.select_channel(6));
        self.br_conv.process(input_block.select_channel(7));

        let left_ch = stereo_output.select_channel_mut(0);
        for (i, v) in left_ch.enumerate() {
            *v = self.fl_conv.left_out[i]
                + self.fr_conv.left_out[i]
                + Self::CENTER_GAIN * self.fc_conv.left_out[i]
                + Self::BACK_GAIN * self.bl_conv.left_out[i]
                + Self::BACK_GAIN * self.br_conv.left_out[i]
                + Self::SIDE_GAIN * self.sl_conv.left_out[i]
                + Self::SIDE_GAIN * self.sr_conv.left_out[i]
                + Self::LFE_GAIN * self.lfe_conv.left_out[i];
        }

        let right_ch = stereo_output.select_channel_mut(1);
        for (i, v) in right_ch.enumerate() {
            *v = self.fl_conv.right_out[i]
                + self.fr_conv.right_out[i]
                + Self::CENTER_GAIN * self.fc_conv.right_out[i]
                + Self::BACK_GAIN * self.bl_conv.right_out[i]
                + Self::BACK_GAIN * self.br_conv.right_out[i]
                + Self::SIDE_GAIN * self.sl_conv.right_out[i]
                + Self::SIDE_GAIN * self.sr_conv.right_out[i]
                + Self::LFE_GAIN * self.lfe_conv.right_out[i];
        }
    }
}
pub struct Equalizer {
    left: BlockConvolver,
    right: BlockConvolver,
    scratch: Vec<f32>,
}

impl Equalizer {
    pub fn new(block_size: usize, eqir: Vec<f32>) -> Self {
        Self {
            left: BlockConvolver::new(block_size, &eqir),
            right: BlockConvolver::new(block_size, &eqir),
            scratch: vec![0.0; block_size],
        }
    }

    pub fn process(&mut self, stereo_data: &mut AudioDataMut<'_>) {
        stereo_data.copy_channel_to_slice(0, &mut self.scratch);
        self.left.process(&mut self.scratch);
        stereo_data.copy_channel_from_slice(0, &self.scratch);

        stereo_data.copy_channel_to_slice(1, &mut self.scratch);
        self.right.process(&mut self.scratch);
        stereo_data.copy_channel_from_slice(1, &self.scratch);
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
