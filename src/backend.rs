use crate::{
    audio_data::{AFrame, AudioDataMut, AudioDataRef},
    audio_swapchain::AudioSwapchain,
    config::{self, AppConfig, AudioSourceMode, EqualizerProfile},
    coremotion, execute_sampled,
    surround_virtualizer::{Equalizer, SpeakerPosition, SurroundVirtualizer, SurroundVirtualizerConfig, wav_to_pcm},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{info, warn};
use num_traits::FromPrimitive;
use ringbuf::traits::Split;
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
const AIRPODS4_EQ: &[u8] = include_bytes!("../res/eq/airpods4.wav");
const K702_EQ: &[u8] = include_bytes!("../res/eq/k702.wav");
const DT770PRO_EQ: &[u8] = include_bytes!("../res/eq/dt770pro.wav");

const NUM_SURROUND_CHANNELS: u32 = 8;
const CH_BUF_SIZE: usize = 2048;
const NUM_OUT_CHANNELS: usize = 2;
const HRIR_SAMPLE_RATE: u32 = 48000;
const AUDIO_BACKEND_TIMEOUT_MS: u64 = 1000;
pub const DEFAULT_INPUT_DEVICE_NAME: &str = "BlackHole 16ch";
pub const DEFAULT_OUTPUT_DEVICE_NAME: &str = "External Headphones";

static RELOAD_NEEDED: AtomicBool = AtomicBool::new(false);
static CURRENT_SOURCE_MODE: AtomicU32 = AtomicU32::new(0);
static CURRENT_EQ_PROFILE: AtomicU32 = AtomicU32::new(0);

pub fn get_input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    device
                        .description()
                        .map(|desc| desc.name().to_string())
                        .ok()
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn get_output_device_names() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    device
                        .description()
                        .map(|desc| desc.name().to_string())
                        .ok()
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn reload_backend() {
    RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);
}

pub fn set_equalizer_profile(profile: EqualizerProfile) {
    CURRENT_EQ_PROFILE.store(profile as u32, atomic::Ordering::Relaxed);
}

pub fn set_source_mode(source_mode: AudioSourceMode) {
    CURRENT_SOURCE_MODE.store(source_mode as u32, atomic::Ordering::Relaxed);
}

fn initiate_reload() {
    thread::sleep(std::time::Duration::from_secs(1));
    RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);
}

fn get_devices(
    host: &cpal::Host,
    config: &AppConfig,
) -> Result<(cpal::Device, cpal::Device), String> {
    let input_device_name = config
        .input_device_name
        .as_deref()
        .unwrap_or(DEFAULT_INPUT_DEVICE_NAME);
    let output_device_name = config
        .output_device_name
        .as_deref()
        .unwrap_or(DEFAULT_OUTPUT_DEVICE_NAME);

    let input_dev = host.input_devices().unwrap().find(|dev| {
        dev.description()
            .map(|desc| desc.name() == input_device_name)
            .unwrap_or(false)
    });
    let Some(input_dev) = input_dev else {
        return Err(format!("Input device '{}' not found", input_device_name));
    };

    let output_dev = host.output_devices().unwrap().find(|dev| {
        dev.description()
            .map(|desc| desc.name() == output_device_name)
            .unwrap_or(false)
    });
    let Some(output_dev) = output_dev else {
        return Err(format!("Output device '{}' not found", output_device_name));
    };

    Ok((input_dev, output_dev))
}

fn start_backend(
    input_dev: &cpal::Device,
    output_dev: &cpal::Device,
) -> Option<(cpal::Stream, cpal::Stream)> {
    let in_dev_name = input_dev
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_default();
    let out_dev_name = output_dev
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_default();

    let virt_config = SurroundVirtualizerConfig {
        speaker_positions: vec![
            SpeakerPosition { angle_degrees: 50.0, hrir_wav: FL_WAV },
            SpeakerPosition { angle_degrees: -50.0, hrir_wav: FR_WAV },
            SpeakerPosition { angle_degrees: 0.0, hrir_wav: FC_WAV },
            SpeakerPosition { angle_degrees: 90.0, hrir_wav: SL_WAV },
            SpeakerPosition { angle_degrees: -90.0, hrir_wav: SR_WAV },
            SpeakerPosition { angle_degrees: 140.0, hrir_wav: BL_WAV },
            SpeakerPosition { angle_degrees: -140.0, hrir_wav: BR_WAV },
        ],
        lfe_wav: LFE_WAV,
        block_size: CH_BUF_SIZE,
    };
    let mut sv = SurroundVirtualizer::new(virt_config);

    let mut eq_earpods = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(EARPODS_EQ));
    let mut eq_airpods4 = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(AIRPODS4_EQ));
    let mut eq_k702 = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(K702_EQ));
    let mut eq_dt770pro = Equalizer::new(CH_BUF_SIZE, wav_to_pcm(DT770PRO_EQ));

    let input_selection = input_dev
        .supported_input_configs()
        .unwrap()
        .filter(|conf| {
            (conf.min_sample_rate() <= HRIR_SAMPLE_RATE)
                && (conf.max_sample_rate() >= HRIR_SAMPLE_RATE)
        })
        .map(|conf| {
            let buf_sz = match conf.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => {
                    CH_BUF_SIZE.clamp(*min as usize, *max as usize)
                }
                _ => CH_BUF_SIZE,
            };
            let ch = conf.channels();
            (buf_sz, ch)
        })
        .min_by_key(|(buf_sz, ch)| {
            let dist_ch = (*ch as isize - NUM_SURROUND_CHANNELS as isize).abs();
            (dist_ch, (*buf_sz as isize - CH_BUF_SIZE as isize).abs())
        });

    let output_buf_size = output_dev
        .supported_output_configs()
        .unwrap()
        .filter(|conf| {
            conf.channels() >= NUM_OUT_CHANNELS as u16
                && (conf.min_sample_rate() <= HRIR_SAMPLE_RATE)
                && (conf.max_sample_rate() >= HRIR_SAMPLE_RATE)
        })
        .map(|conf| match conf.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => {
                CH_BUF_SIZE.clamp(*min as usize, *max as usize)
            }
            _ => CH_BUF_SIZE,
        })
        .min_by_key(|buf_size| (*buf_size as isize - CH_BUF_SIZE as isize).abs());

    let Some((input_buf_size, in_selected_channels)) = input_selection else {
        warn!("Error: No supported input config found for device '{in_dev_name}'",);
        initiate_reload();
        return None;
    };
    let Some(output_buf_size) = output_buf_size else {
        warn!("Error: No supported output config found for device '{out_dev_name}'",);
        initiate_reload();
        return None;
    };

    let in_config = cpal::StreamConfig {
        channels: in_selected_channels.min(NUM_SURROUND_CHANNELS as u16),
        sample_rate: HRIR_SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Fixed(input_buf_size as u32),
    };

    let out_config = cpal::StreamConfig {
        channels: NUM_OUT_CHANNELS as u16,
        sample_rate: HRIR_SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Fixed(output_buf_size as u32),
    };

    let in_sw = Arc::new(AudioSwapchain::new(
        input_buf_size * in_config.channels as usize,
        CH_BUF_SIZE * in_config.channels as usize,
        1,
    ));
    let (mut in_rb_prod, mut in_rb_cons) =
        ringbuf::HeapRb::<AFrame<{ NUM_SURROUND_CHANNELS as usize }>>::new(
            in_sw.desired_rb_size() / in_config.channels as usize,
        )
        .split();

    let out_sw = Arc::new(AudioSwapchain::new(
        CH_BUF_SIZE * NUM_OUT_CHANNELS as usize,
        output_buf_size * NUM_OUT_CHANNELS as usize,
        3,
    ));
    let (mut out_rb_prod, mut out_rb_cons) = ringbuf::HeapRb::<AFrame<NUM_OUT_CHANNELS>>::new(
        out_sw.desired_rb_size() / NUM_OUT_CHANNELS,
    )
    .split();

    // first create the output stream to reduce glitches at startup
    let aq = Arc::clone(&out_sw);
    let out_stream = output_dev
        .build_output_stream(
            &out_config,
            move |output: &mut [f32], _| {
                let Some(buf) = aq.acquire_ready_output_buf(&mut out_rb_cons) else {
                    output.fill(cpal::Sample::EQUILIBRIUM);
                    return;
                };

                if output.len() != buf.data().len() {
                    execute_sampled!(Duration::from_secs(5), {
                        warn!(
                            "Output buffer size mismatch: expected {}, got {}",
                            buf.data().len(),
                            output.len()
                        );
                    });
                    initiate_reload();
                    return;
                }

                output.copy_from_slice(buf.data());
            },
            move |err| {
                warn!("Output error: {}", err);
                initiate_reload();
            },
            Some(Duration::from_millis(AUDIO_BACKEND_TIMEOUT_MS)),
        )
        .unwrap();

    let aq = Arc::clone(&out_sw);
    let in_stream = input_dev
        .build_input_stream(
            &in_config,
            move |input: &[f32], _| {
                let num_frames_pushed = AudioSwapchain::submit_input(input, &mut in_rb_prod);
                if num_frames_pushed < input.len() / in_config.channels as usize {
                    execute_sampled!(Duration::from_secs(5), {
                        warn!(
                            "Warning: dropped {} frames due to full input ringbuffer",
                            (input.len() / in_config.channels as usize) - num_frames_pushed
                        );
                    });
                }

                let Some(input) = in_sw.acquire_ready_output_buf(&mut in_rb_cons) else {
                    return;
                };

                let Some(mut buf) = aq.acquire_free_input_buf() else {
                    return;
                };

                let in_ch = in_config.channels as usize;
                let input_adata = AudioDataRef::new(input.data(), in_ch);
                let mut stereo_adata =
                    AudioDataMut::new(buf.data_mut(), out_config.channels as usize);

                let head_yaw = coremotion::get_head_yaw();
                let head_pitch = coremotion::get_head_pitch();

                let current_source_mode = CURRENT_SOURCE_MODE.load(atomic::Ordering::Relaxed);
                match AudioSourceMode::from_u32(current_source_mode)
                    .unwrap_or(AudioSourceMode::Universal)
                {
                    AudioSourceMode::Universal => {
                        if in_ch >= NUM_SURROUND_CHANNELS as usize {
                            sv.process_ch8(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                        } else if in_ch >= 2 {
                            sv.process_ch2(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                        } else {
                            sv.process_mono(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                        }
                    }
                    AudioSourceMode::Stereo => {
                        if in_ch >= 2 {
                            sv.process_ch2(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                        } else {
                            sv.process_mono(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                        }
                    }
                    AudioSourceMode::Mono => {
                        sv.process_mono(&input_adata, &mut stereo_adata, head_yaw, head_pitch);
                    }
                }

                let current_profile = CURRENT_EQ_PROFILE.load(atomic::Ordering::Relaxed);
                match EqualizerProfile::from_u32(current_profile).unwrap_or(EqualizerProfile::None)
                {
                    EqualizerProfile::EarPods => eq_earpods.process(&mut stereo_adata),
                    EqualizerProfile::AirPods4 => eq_airpods4.process(&mut stereo_adata),
                    EqualizerProfile::K702 => eq_k702.process(&mut stereo_adata),
                    EqualizerProfile::DT770Pro => eq_dt770pro.process(&mut stereo_adata),
                    _ => {}
                }

                let num_frames_pushed = AudioSwapchain::submit_input(buf.data(), &mut out_rb_prod);
                if num_frames_pushed < buf.data().len() / NUM_OUT_CHANNELS {
                    execute_sampled!(Duration::from_secs(5), {
                        warn!(
                            "Warning: dropped {} frames due to full output ringbuffer",
                            (buf.data().len() / NUM_OUT_CHANNELS) - num_frames_pushed
                        );
                    });
                }
            },
            move |err| {
                warn!("Input error: {}", err);
                initiate_reload();
            },
            Some(Duration::from_millis(AUDIO_BACKEND_TIMEOUT_MS)),
        )
        .unwrap();

    let _ = out_stream.play();
    let _ = in_stream.play();

    Some((in_stream, out_stream))
}

pub fn run() {
    RELOAD_NEEDED.store(true, atomic::Ordering::Relaxed);

    let host = cpal::default_host();
    let mut in_stream = None;
    let mut out_stream = None;

    thread::spawn(|| unsafe {
        coremotion::start_head_tracking();
    });

    loop {
        let conf = config::get_snapshot();
        let devices = get_devices(&host, &conf);
        let do_reload = RELOAD_NEEDED.swap(false, atomic::Ordering::Relaxed);

        if do_reload {
            drop(in_stream.take());
            drop(out_stream.take());
        }

        if let Err(str) = devices {
            execute_sampled!(Duration::from_secs(5), {
                warn!("{}", str);
            });
            initiate_reload();
            continue;
        }
        let (input_dev, output_dev) = devices.unwrap();

        if do_reload {
            info!("Starting backend...");
            if let Some((in_str, out_str)) = start_backend(&input_dev, &output_dev) {
                in_stream = Some(in_str);
                out_stream = Some(out_str);
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}
