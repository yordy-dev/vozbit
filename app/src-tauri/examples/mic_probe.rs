//! Manual, opt-in hardware diagnostic. Never writes or prints captured audio.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn measure<T: cpal::SizedSample + Copy + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
) -> Result<(u64, u64), String> {
    let frames = Arc::new(AtomicU64::new(0));
    let nonzero = Arc::new(AtomicU64::new(0));
    let frame_counter = frames.clone();
    let signal_counter = nonzero.clone();
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                frame_counter.fetch_add((data.len() / channels) as u64, Ordering::Relaxed);
                // Generic formats are counted as delivered, not inspected or persisted.
                if !data.is_empty() {
                    signal_counter.fetch_add(1, Ordering::Relaxed);
                }
            },
            |error| eprintln!("stream error: {error}"),
            None,
        )
        .map_err(|error| error.to_string())?;
    stream.play().map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_secs(2));
    drop(stream);
    Ok((
        frames.load(Ordering::Relaxed),
        nonzero.load(Ordering::Relaxed),
    ))
}

fn main() {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        println!("NO_DEFAULT_INPUT_DEVICE");
        return;
    };
    println!("DEFAULT_INPUT_PRESENT");
    let Ok(default) = device.default_input_config() else {
        println!("DEFAULT_CONFIG_UNAVAILABLE");
        return;
    };
    println!(
        "DEFAULT rate={} channels={} format={}",
        default.sample_rate(),
        default.channels(),
        default.sample_format()
    );
    let Ok(ranges) = device.supported_input_configs() else {
        println!("SUPPORTED_CONFIGS_UNAVAILABLE");
        return;
    };
    let ranges: Vec<_> = ranges.collect();
    for rate in [44_100, 48_000] {
        let choice = ranges.iter().find(|range| {
            range.min_sample_rate() <= rate
                && rate <= range.max_sample_rate()
                && matches!(
                    range.sample_format(),
                    SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
                )
        });
        let Some(choice) = choice else {
            println!("RATE {rate}: UNSUPPORTED");
            continue;
        };
        let supported = choice.clone().with_sample_rate(rate);
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let result = match format {
            SampleFormat::F32 => measure::<f32>(&device, &config),
            SampleFormat::I16 => measure::<i16>(&device, &config),
            SampleFormat::U16 => measure::<u16>(&device, &config),
            _ => unreachable!(),
        };
        match result {
            Ok((frames, chunks)) => println!("RATE {rate}: frames={frames} chunks={chunks}"),
            Err(error) => println!("RATE {rate}: ERROR {error}"),
        }
    }
}
