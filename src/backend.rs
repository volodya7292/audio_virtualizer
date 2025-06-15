use crate::{
    audio_buffer_queue::AudioBufferQueue,
    surround_virtualizer::{SurroundVirtualizer, SurroundVirtualizerConfig},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, mpsc};

const FC_WAV: &[u8] = include_bytes!("../res/FC.wav");
const BL_WAV: &[u8] = include_bytes!("../res/BL.wav");
const BR_WAV: &[u8] = include_bytes!("../res/BR.wav");
const FL_WAV: &[u8] = include_bytes!("../res/FL.wav");
const FR_WAV: &[u8] = include_bytes!("../res/FR.wav");
const SL_WAV: &[u8] = include_bytes!("../res/SL.wav");
const SR_WAV: &[u8] = include_bytes!("../res/SR.wav");
const LFE_WAV: &[u8] = include_bytes!("../res/LFE.wav");
const NUM_SURROUND_CHANNELS: u32 = 8;
const CH_BUF_SIZE: usize = 2048;
const HRIR_SAMPLE_RATE: u32 = 48000;
const INPUT_DEVICE_NAME: &str = "BlackHole 16ch";
const OUTPUT_DEVICE_NAME: &str = "External Headphones";

fn start_backend(
    host: &cpal::Host,
    reload_tx: &mpsc::SyncSender<()>,
    in_stream_var: &mut Option<cpal::Stream>,
    out_stream_var: &mut Option<cpal::Stream>,
) {
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
    let mut sv = SurroundVirtualizer::new(&virt_config);

    let input_dev = host.input_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == INPUT_DEVICE_NAME)
            .unwrap_or(false)
    });
    let Some(input_dev) = input_dev else {
        println!("Input device not found");
        std::thread::sleep(std::time::Duration::from_secs(1));
        reload_tx.send(()).unwrap();
        return;
    };

    let output_dev = host.output_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == OUTPUT_DEVICE_NAME)
            .unwrap_or(false)
    });
    let Some(output_dev) = output_dev else {
        println!("Output device not found");
        std::thread::sleep(std::time::Duration::from_secs(1));
        reload_tx.send(()).unwrap();
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
    let rel_tx = reload_tx.clone();
    let in_stream = input_dev
        .build_input_stream(
            &in_config,
            move |input: &[f32], _| {
                let Some(mut buf) = aq.acquire_free_buf() else {
                    return;
                };
                sv.process(input, in_config.channels as usize, &mut buf);
                aq.submit_buf(buf);
            },
            move |err| {
                eprintln!("Input error: {}", err);
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = rel_tx.send(());
            },
            None,
        )
        .unwrap();

    let aq = Arc::clone(&audio_queue);
    let rel_tx = reload_tx.clone();
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
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = rel_tx.send(());
            },
            None,
        )
        .unwrap();

    let _ = in_stream.play();
    let _ = out_stream.play();

    in_stream_var.replace(in_stream);
    out_stream_var.replace(out_stream);
}

pub fn run() {
    let (reload_tx, reload_rx) = std::sync::mpsc::sync_channel(1);
    reload_tx.send(()).unwrap();

    let host = cpal::default_host();
    let mut in_stream = None;
    let mut out_stream = None;

    while let Ok(_) = reload_rx.recv() {
        println!("Starting backend...");
        start_backend(&host, &reload_tx, &mut in_stream, &mut out_stream);
    }

    println!("Backend stopped.");
}
