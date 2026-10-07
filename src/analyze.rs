use std::sync::mpsc;

use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait};

use spectrum_analyzer::{FiniteF32, FrequencySpectrum, samples_fft_to_spectrum};
use spectrum_analyzer::scaling::scale_20_times_log10;
use spectrum_analyzer::windows::hann_window;

pub fn analyze_global(analyzer_tx: mpsc::Sender<Vec<FiniteF32>>) -> Result<cpal::Stream, cpal::Error> {
    let host = cpal::default_host();
    let device_list: Vec<cpal::Device> = host.output_devices().expect("no output devices available").collect();
    // TODO let user choose audio device; WIP code commented out here
    // for dev in &device_list {
    //     println!("{}", dev);
    // };
    // let mut device_id_string = String::new();
    // io::stdin().read_line(&mut device_id_string).expect("please select an output device");
    // let device_id_number: usize = device_id_string.trim().parse().expect("invalid device id");
    // let device = &device_list[device_id_number - 1];

    let device = &device_list[0];
    println!("selected: {}", device);
    println!("supports output: {}", device.supports_output());
    println!("supports input: {}", device.supports_input());

    // let mut supported_configs_range = device.supported_output_configs()
    //     .expect("error while querying configs");
    // let supported_config = supported_configs_range.next()
    //     .expect("no supported config?!")
    //     .with_max_sample_rate();
    let supported_config = device.default_output_config().expect("no default output config");

    let buffer = supported_config.buffer_size();
    let config: StreamConfig = supported_config.into();
    let channels = config.channels as usize;

    println!("buffer: {:?}", buffer);
    println!("channels: {:?}", config.channels);
    println!("Sample Rate: {:?}", config.sample_rate);
    println!("Sample Format: {:?}", supported_config.sample_format());

    let n = 1024;
    let mut sample_buffer = Vec::with_capacity(n);

    let analyzer_tx_handle = analyzer_tx.clone();

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            for chunk in data.chunks_exact(channels) {
                let mono_sample = chunk.iter().sum::<f32>() / channels as f32;
                sample_buffer.push(mono_sample);

                if sample_buffer.len() == n {
                    let windowed_samples = hann_window(&sample_buffer);
                    let spectrum = samples_fft_to_spectrum(
                        &windowed_samples,
                        config.sample_rate,
                        spectrum_analyzer::FrequencyLimit::All,
                        Some(&scale_20_times_log10)
                    ).unwrap();

                    let bins = create_bins(spectrum);

                    let _ = analyzer_tx_handle.send(bin_maximums(bins));
                    
                    sample_buffer.clear();
                }
            }
        },
        move |err| {
            eprintln!("an error occurred on the input audio stream: {}", err);
        },
        None
    );

    return stream
}

fn create_bins(spectrum: FrequencySpectrum) -> Vec<((f32, f32), Vec<FiniteF32>)> {
    let mut bins = vec![((0_f32, 20_f32), Vec::new()), ((20_f32, 30_f32), Vec::new()),
                        ((30_f32, 60_f32), Vec::new()), ((60_f32, 120_f32), Vec::new()),
                        ((120_f32, 280_f32), Vec::new()), ((280_f32, 700_f32), Vec::new()),
                        ((700_f32, 1500_f32), Vec::new()), ((1500_f32, 3000_f32), Vec::new()),
                        ((3000_f32, 7000_f32), Vec::new()), ((7000_f32, 15000_f32), Vec::new()),
                        ((15000_f32, 99999_f32), Vec::new())];
    
    for datum in spectrum.data() {
        for bin in &mut bins {
            if datum.0 >= bin.0.0 && datum.0 < bin.0.1 {
                bin.1.push(datum.1);
            }
        }
    }

    return bins
}

fn bin_maximums (bins: Vec<((f32, f32), Vec<FiniteF32>)>) -> Vec<FiniteF32> {
    let mut maximums = Vec::new();
    for bin in bins {
        maximums.push(bin.1.into_iter().max().unwrap_or(0.0_f32.into()));
    }
    return maximums
}
