use crate::{
    audio_swapchain::AudioSwapchain,
    config::{self, EqualizerProfile},
    surround_virtualizer::{Equalizer, SurroundVirtualizer, SurroundVirtualizerConfig, wav_to_pcm},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use num_traits::FromPrimitive;
use std::{
    sync::{
        Arc,
        atomic::{self, AtomicBool, AtomicU32},
    },
    thread,
    time::Duration,
};

const FC_WAV: &[u8] = include_bytes!("../res/hrir/1/FC.wav");
const BL_WAV: &[u8] = include_bytes!("../res/hrir/1/BL.wav");
const BR_WAV: &[u8] = include_bytes!("../res/hrir/1/BR.wav");
const FL_WAV: &[u8] = include_bytes!("../res/hrir/1/FL.wav");
const FR_WAV: &[u8] = include_bytes!("../res/hrir/1/FR.wav");
const SL_WAV: &[u8] = include_bytes!("../res/hrir/1/SL.wav");
const SR_WAV: &[u8] = include_bytes!("../res/hrir/1/SR.wav");
const LFE_WAV: &[u8] = include_bytes!("../res/hrir/1/LFE.wav");

const EARPODS_EQ: &[u8] = include_bytes!("../res/eq/earpods.wav");
const K702_EQ: &[u8] = include_bytes!("../res/eq/k702.wav");
const DT770PRO_EQ: &[u8] = include_bytes!("../res/eq/dt770pro.wav");

const NUM_SURROUND_CHANNELS: u32 = 8;
const CH_BUF_SIZE: usize = 2048;
const NUM_OUT_CHANNELS: usize = 2;
const HRIR_SAMPLE_RATE: u32 = 48000;
pub const DEFAULT_INPUT_DEVICE_NAME: &str = "BlackHole 16ch";
pub const DEFAULT_OUTPUT_DEVICE_NAME: &str = "External Headphones";

static RELOAD_NEEDED: AtomicBool = AtomicBool::new(false);
static CURRENT_EQ_PROFILE: AtomicU32 = AtomicU32::new(0);

pub fn get_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

pub fn get_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

pub fn reload_backend() {
    RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);
}

pub fn set_equalizer_profile(profile: EqualizerProfile) {
    CURRENT_EQ_PROFILE.store(profile as u32, atomic::Ordering::Relaxed);
}

fn start_backend(
    host: &cpal::Host,
    in_stream_var: &mut Option<cpal::Stream>,
    out_stream_var: &mut Option<cpal::Stream>,
) {
    let config = config::get_snapshot();

    let input_device_name = config
        .input_device_name
        .as_deref()
        .unwrap_or(DEFAULT_INPUT_DEVICE_NAME);
    let output_device_name = config
        .output_device_name
        .as_deref()
        .unwrap_or(DEFAULT_OUTPUT_DEVICE_NAME);

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

    let mut eq_earpods = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(EARPODS_EQ));
    let mut eq_k702 = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(K702_EQ));
    let mut eq_dt770pro = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(DT770PRO_EQ));

    let reload_fn = move || {
        thread::sleep(std::time::Duration::from_secs(1));
        RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);
    };

    let input_dev = host.input_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == input_device_name)
            .unwrap_or(false)
    });
    let Some(input_dev) = input_dev else {
        println!("Input device '{}' not found", input_device_name);
        reload_fn();
        return;
    };

    let output_dev = host.output_devices().unwrap().find(|dev| {
        dev.name()
            .map(|name| name == output_device_name)
            .unwrap_or(false)
    });
    let Some(output_dev) = output_dev else {
        println!("Output device '{}' not found", output_device_name);
        reload_fn();
        return;
    };

    output_dev.as_inner();

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

    let audio_queue = Arc::new(AudioSwapchain::new(CH_BUF_SIZE * NUM_OUT_CHANNELS, 3));

    let aq = Arc::clone(&audio_queue);
    let in_stream = input_dev
        .build_input_stream(
            &in_config,
            move |input: &[f32], _| {
                let Some(mut buf) = aq.acquire_free_buf() else {
                    return;
                };
                sv.process(input, in_config.channels as usize, &mut buf);

                let current_profile = CURRENT_EQ_PROFILE.load(atomic::Ordering::Relaxed);
                match EqualizerProfile::from_u32(current_profile).unwrap() {
                    EqualizerProfile::Earpods => eq_earpods.process(&mut buf),
                    EqualizerProfile::K702 => eq_k702.process(&mut buf),
                    EqualizerProfile::DT770Pro => eq_dt770pro.process(&mut buf),
                    _ => {}
                }

                aq.submit_buf(buf);
            },
            move |err| {
                eprintln!("Input error: {}", err);
                reload_fn();
            },
            None,
        )
        .unwrap();

    let aq = Arc::clone(&audio_queue);
    let out_stream = output_dev
        .build_output_stream(
            &out_config,
            move |output: &mut [f32], _| {
                let Some(buf) = aq.acquire_ready_buf() else {
                    for buf in output.iter_mut() {
                        *buf = cpal::Sample::EQUILIBRIUM;
                    }
                    return;
                };
                output.copy_from_slice(&buf);
                aq.release_buf(buf);
            },
            move |err| {
                eprintln!("Output error: {}", err);
                reload_fn();
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
    RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);

    let host = cpal::default_host();
    let mut in_stream = None;
    let mut out_stream = None;

    loop {
        if RELOAD_NEEDED.swap(false, atomic::Ordering::Relaxed) {
            println!("Starting backend...");
            drop(in_stream.take());
            drop(out_stream.take());
            start_backend(&host, &mut in_stream, &mut out_stream);
        }

        thread::sleep(Duration::from_millis(100));
    }
}
