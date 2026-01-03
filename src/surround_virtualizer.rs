use crate::audio_data::{AudioDataMut, AudioDataRef};
use crate::block_convolver::BlockConvolver;
use std::io::Cursor;

pub struct SurroundVirtualizerConfig {
    pub speaker_positions: Vec<SpeakerPosition>,
    pub lfe_wav: &'static [u8],
    pub block_size: usize,
}
pub struct SpeakerPosition {
    pub angle_degrees: f32,
    pub hrir_wav: &'static [u8],
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
    speaker_convolvers: Vec<Vec<BinauralConvolver>>,
    hrir_angles: Vec<f32>,
    lfe_conv: BinauralConvolver,
    prev_yaw: f32,
    pitch_filter: PitchFilter,
}

impl SurroundVirtualizer {
    pub fn new(config: SurroundVirtualizerConfig) -> Self {
        let hrir_angles: Vec<f32> = config
            .speaker_positions
            .iter()
            .map(|pos| pos.angle_degrees)
            .collect();

        let n_speakers = 7;
        let speaker_convolvers: Vec<Vec<BinauralConvolver>> = (0..n_speakers)
            .map(|_| {
                config
                    .speaker_positions
                    .iter()
                    .map(|pos| wav_to_binaural_convolver(pos.hrir_wav, config.block_size))
                    .collect()
            })
            .collect();

        let lfe_conv = wav_to_binaural_convolver(config.lfe_wav, config.block_size);

        Self {
            block_size: config.block_size,
            speaker_convolvers,
            hrir_angles,
            lfe_conv,
            prev_yaw: 0.0,
            pitch_filter: PitchFilter::new(),
        }
    }

    fn find_nearest_hrirs(&self, target_angle: f32) -> (usize, usize, f32) {
        let normalized = ((target_angle % 360.0) + 360.0) % 360.0;

        let mut min_dist = f32::MAX;
        let mut idx0 = 0;

        for (i, &angle) in self.hrir_angles.iter().enumerate() {
            let norm_hrir = ((angle % 360.0) + 360.0) % 360.0;
            let dist = (normalized - norm_hrir)
                .abs()
                .min(360.0 - (normalized - norm_hrir).abs());
            if dist < min_dist {
                min_dist = dist;
                idx0 = i;
            }
        }

        let angle0 = ((self.hrir_angles[idx0] % 360.0) + 360.0) % 360.0;
        let mut next_dist = f32::MAX;
        let mut idx1 = idx0;

        for (i, &angle) in self.hrir_angles.iter().enumerate() {
            if i == idx0 {
                continue;
            }
            let norm_hrir = ((angle % 360.0) + 360.0) % 360.0;
            let dist = (normalized - norm_hrir)
                .abs()
                .min(360.0 - (normalized - norm_hrir).abs());
            if dist < next_dist {
                next_dist = dist;
                idx1 = i;
            }
        }

        let angle1 = ((self.hrir_angles[idx1] % 360.0) + 360.0) % 360.0;
        let total_span = (angle1 - angle0).abs().min(360.0 - (angle1 - angle0).abs());
        let frac = if total_span < 1e-6 {
            0.0
        } else {
            let dist_from_0 = (normalized - angle0)
                .abs()
                .min(360.0 - (normalized - angle0).abs());
            dist_from_0 / total_span
        };

        (idx0, idx1, frac)
    }

    pub fn process_ch8(
        &mut self,
        input_block: &AudioDataRef,
        stereo_output: &mut AudioDataMut,
        yaw: f32,
        pitch: f32,
    ) {
        assert_eq!(stereo_output.data.len(), self.block_size * 2);

        const CENTER_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
        const SIDE_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
        const BACK_GAIN: f32 = 0.5 * std::f32::consts::SQRT_2;
        const LFE_GAIN: f32 = 0.25;

        let gains = [
            1.0,
            1.0,
            CENTER_GAIN,
            SIDE_GAIN,
            SIDE_GAIN,
            BACK_GAIN,
            BACK_GAIN,
        ];
        let yaw_deg = yaw.to_degrees();

        self.lfe_conv.process(input_block.select_channel(3));

        for i in 0..self.block_size {
            let l = LFE_GAIN * self.lfe_conv.left_out[i];
            let r = LFE_GAIN * self.lfe_conv.right_out[i];

            stereo_output.data[i * 2] = l;
            stereo_output.data[i * 2 + 1] = r;
        }

        let channels = [0, 1, 2, 4, 5, 6, 7];

        for (ch_idx, &ch) in channels.iter().enumerate() {
            let speaker_angle = self.hrir_angles[ch_idx];
            let rotated_angle = speaker_angle - yaw_deg;
            let (idx0, idx1, frac) = self.find_nearest_hrirs(rotated_angle);

            self.speaker_convolvers[ch_idx][idx0].process(input_block.select_channel(ch));
            self.speaker_convolvers[ch_idx][idx1].process(input_block.select_channel(ch));

            let gain = gains[ch_idx];
            for i in 0..self.block_size {
                let interp_l = (1.0 - frac) * self.speaker_convolvers[ch_idx][idx0].left_out[i]
                    + frac * self.speaker_convolvers[ch_idx][idx1].left_out[i];
                let interp_r = (1.0 - frac) * self.speaker_convolvers[ch_idx][idx0].right_out[i]
                    + frac * self.speaker_convolvers[ch_idx][idx1].right_out[i];

                stereo_output.data[i * 2] += gain * interp_l;
                stereo_output.data[i * 2 + 1] += gain * interp_r;
            }
        }

        self.pitch_filter.update_coeffs(pitch);
        self.pitch_filter
            .process_interleaved(&mut stereo_output.data);

        self.prev_yaw = yaw_deg;
    }

    pub fn process_ch2(
        &mut self,
        input_block: &AudioDataRef,
        stereo_output: &mut AudioDataMut,
        yaw: f32,
        pitch: f32,
    ) {
        assert_eq!(stereo_output.data.len(), self.block_size * 2);

        const FRONT_GAIN: f32 = 0.8;
        const SIDE_GAIN: f32 = 0.4;

        let yaw_deg = yaw.to_degrees();

        for i in 0..self.block_size {
            stereo_output.data[i * 2] = 0.0;
            stereo_output.data[i * 2 + 1] = 0.0;
        }

        let virtual_speakers = [
            (30.0, 0, FRONT_GAIN),
            (-30.0, 1, FRONT_GAIN),
            (110.0, 0, SIDE_GAIN),
            (-110.0, 1, SIDE_GAIN),
        ];

        for (virt_spk_idx, &(speaker_angle, ch, gain)) in virtual_speakers.iter().enumerate() {
            let rotated_angle = speaker_angle - yaw_deg;
            let (idx0, idx1, frac) = self.find_nearest_hrirs(rotated_angle);

            let spk_idx = if virt_spk_idx < 2 {
                virt_spk_idx
            } else {
                virt_spk_idx + 1
            };

            self.speaker_convolvers[spk_idx][idx0].process(input_block.select_channel(ch));
            self.speaker_convolvers[spk_idx][idx1].process(input_block.select_channel(ch));

            for i in 0..self.block_size {
                let interp_l = (1.0 - frac) * self.speaker_convolvers[spk_idx][idx0].left_out[i]
                    + frac * self.speaker_convolvers[spk_idx][idx1].left_out[i];
                let interp_r = (1.0 - frac) * self.speaker_convolvers[spk_idx][idx0].right_out[i]
                    + frac * self.speaker_convolvers[spk_idx][idx1].right_out[i];

                stereo_output.data[i * 2] += gain * interp_l;
                stereo_output.data[i * 2 + 1] += gain * interp_r;
            }
        }

        self.pitch_filter.update_coeffs(pitch);
        self.pitch_filter
            .process_interleaved(&mut stereo_output.data);

        self.prev_yaw = yaw_deg;
    }

    pub fn process_mono(
        &mut self,
        mono_input: &AudioDataRef,
        stereo_output: &mut AudioDataMut,
        yaw: f32,
        pitch: f32,
    ) {
        assert_eq!(stereo_output.data.len(), self.block_size * 2);

        let yaw_deg = yaw.to_degrees();
        let speaker_angles_mono = [30.0, -30.0];

        for i in 0..self.block_size {
            stereo_output.data[i * 2] = 0.0;
            stereo_output.data[i * 2 + 1] = 0.0;
        }

        for (spk_idx, &speaker_angle) in speaker_angles_mono.iter().enumerate() {
            let rotated_angle = speaker_angle - yaw_deg;
            let (idx0, idx1, frac) = self.find_nearest_hrirs(rotated_angle);

            self.speaker_convolvers[spk_idx][idx0].process(mono_input.select_channel(0));
            self.speaker_convolvers[spk_idx][idx1].process(mono_input.select_channel(0));

            for i in 0..self.block_size {
                let interp_l = (1.0 - frac) * self.speaker_convolvers[spk_idx][idx0].left_out[i]
                    + frac * self.speaker_convolvers[spk_idx][idx1].left_out[i];
                let interp_r = (1.0 - frac) * self.speaker_convolvers[spk_idx][idx0].right_out[i]
                    + frac * self.speaker_convolvers[spk_idx][idx1].right_out[i];

                stereo_output.data[i * 2] += interp_l;
                stereo_output.data[i * 2 + 1] += interp_r;
            }
        }

        self.pitch_filter.update_coeffs(pitch);
        self.pitch_filter
            .process_interleaved(&mut stereo_output.data);

        self.prev_yaw = yaw_deg;
    }
}

struct PitchFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1_l: f32,
    x2_l: f32,
    y1_l: f32,
    y2_l: f32,
    x1_r: f32,
    x2_r: f32,
    y1_r: f32,
    y2_r: f32,
}

impl PitchFilter {
    fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1_l: 0.0,
            x2_l: 0.0,
            y1_l: 0.0,
            y2_l: 0.0,
            x1_r: 0.0,
            x2_r: 0.0,
            y1_r: 0.0,
            y2_r: 0.0,
        }
    }

    fn update_coeffs(&mut self, pitch_radians: f32) {
        let gain_db = pitch_radians.to_degrees().clamp(-30.0, 30.0) * 0.1;
        let freq = 5000.0;
        let sample_rate = 48000.0;

        let a = 10_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * 0.707;

        let a_plus = a + 1.0;
        let a_minus = a - 1.0;
        let sqrt_a = a.sqrt() * 2.0 * alpha;

        self.b0 = a * (a_plus + a_minus * cos_w0 + sqrt_a);
        self.b1 = -2.0 * a * (a_minus + a_plus * cos_w0);
        self.b2 = a * (a_plus + a_minus * cos_w0 - sqrt_a);
        let a0 = a_plus - a_minus * cos_w0 + sqrt_a;
        self.a1 = 2.0 * (a_minus - a_plus * cos_w0);
        self.a2 = a_plus - a_minus * cos_w0 - sqrt_a;

        self.b0 /= a0;
        self.b1 /= a0;
        self.b2 /= a0;
        self.a1 /= a0;
        self.a2 /= a0;
    }

    fn process_interleaved(&mut self, stereo_data: &mut [f32]) {
        for i in (0..stereo_data.len()).step_by(2) {
            let x_l = stereo_data[i];
            let y_l = self.b0 * x_l + self.b1 * self.x1_l + self.b2 * self.x2_l
                - self.a1 * self.y1_l
                - self.a2 * self.y2_l;
            self.x2_l = self.x1_l;
            self.x1_l = x_l;
            self.y2_l = self.y1_l;
            self.y1_l = y_l;
            stereo_data[i] = y_l;

            let x_r = stereo_data[i + 1];
            let y_r = self.b0 * x_r + self.b1 * self.x1_r + self.b2 * self.x2_r
                - self.a1 * self.y1_r
                - self.a2 * self.y2_r;
            self.x2_r = self.x1_r;
            self.x1_r = x_r;
            self.y2_r = self.y1_r;
            self.y1_r = y_r;
            stereo_data[i + 1] = y_r;
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
