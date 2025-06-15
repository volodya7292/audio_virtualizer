use crate::{
    audio_buffer_queue::AudioBufferQueue,
    surround_virtualizer::{SurroundVirtualizer, SurroundVirtualizerConfig},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{mem, sync::Arc};

const FC_WAV: &'static [u8] = include_bytes!("../res/FC.wav");
const BL_WAV: &'static [u8] = include_bytes!("../res/BL.wav");
const BR_WAV: &'static [u8] = include_bytes!("../res/BR.wav");
const FL_WAV: &'static [u8] = include_bytes!("../res/FL.wav");
const FR_WAV: &'static [u8] = include_bytes!("../res/FR.wav");
const SL_WAV: &'static [u8] = include_bytes!("../res/SL.wav");
const SR_WAV: &'static [u8] = include_bytes!("../res/SR.wav");
const LFE_WAV: &'static [u8] = include_bytes!("../res/LFE.wav");
const NUM_SURROUND_CHANNELS: u32 = 8;
const CH_BUF_SIZE: usize = 2048;
const HRIR_SAMPLE_RATE: u32 = 48000;

pub fn start_processing() {
    let virt_config = SurroundVirtualizerConfig {
        fc_wav: FC_WAV,
        bl_wav: BL_WAV,
        br_wav: BR_WAV,
        fl_wav: FL_WAV,
        fr_wav: FR_WAV,
        sl_wav: SL_WAV,
        sr_wav: SR_WAV,
        lfe_wav: LFE_WAV,
        block_size: CH_BUF_SIZE,
    };
    let mut sv = SurroundVirtualizer::new(virt_config);

    let host = cpal::default_host();

    let output_dev = host.output_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == "External Headphones")
            .unwrap_or(false)
    });
    let Some(output_dev) = output_dev else {
        println!("Output Device not found");
        return;
    };

    let input_dev = host.input_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == "BlackHole 16ch")
            .unwrap_or(false)
    });
    let Some(input_dev) = input_dev else {
        println!("Input Device not found");
        return;
    };

    let in_config = cpal::StreamConfig {
        channels: NUM_SURROUND_CHANNELS as u16,
        sample_rate: cpal::SampleRate(HRIR_SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(CH_BUF_SIZE as u32),
    };

    let out_config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(HRIR_SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(CH_BUF_SIZE as u32),
    };

    let audio_queue = Arc::new(AudioBufferQueue::new(CH_BUF_SIZE * 2));

    let aq = Arc::clone(&audio_queue);
    let in_stream = input_dev
        .build_input_stream(
            &in_config,
            move |input: &[f32], _| {
                let Some(mut buf) = aq.acquire_free_buf() else {
                    println!("No free buffer available");
                    return;
                };
                sv.process(input, in_config.channels as usize, &mut buf);
                aq.submit_buf(buf);
            },
            |err| {
                eprintln!("Input error: {}", err);
            },
            None,
        )
        .unwrap();

    let aq = Arc::clone(&audio_queue);
    let out_stream = output_dev
        .build_output_stream(
            &out_config,
            move |output: &mut [f32], _| {
                let buf = aq.acquire_ready_buf();
                output.copy_from_slice(&buf);
                aq.release_buf(buf);
            },
            move |err| {
                eprintln!("Output error: {}", err);
            },
            None,
        )
        .unwrap();

    in_stream.play().unwrap();
    out_stream.play().unwrap();

    // prevent the streams from being dropped
    mem::forget(in_stream);
    mem::forget(out_stream);
}
