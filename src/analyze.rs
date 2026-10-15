use std::sync::mpsc;
use std::thread;

use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait};

use spectrum_analyzer::{FiniteF32, FrequencySpectrum, NonNegF32, samples_fft_to_spectrum};
use spectrum_analyzer::windows::hann_window;

pub fn analyze_global(analyzer_tx: mpsc::Sender<Vec<FiniteF32>>, mut device_idx: usize) -> Result<cpal::Stream, cpal::Error> {
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

    if device_idx >= device_list.len() {
        eprintln!("Audio Device argument out of bounds! Defaulting to 0");
        device_idx = 0;
    }

    let device = &device_list[device_idx];
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

    // TODO when I tried going up to 2048, was super laggy and cpal was throwing a bunch of buffer
    // underrun/overrun errors. See if there's a way to fix this; lmms has some stuff to do with
    // detecting when processing can't keep up?
    // lmms allows bins from 256 to 16384 - "ranging from barely useful to barely acceptable because
    // of performance and latency reasons" - default is 2048
    // WAIT FOUND IT WINDOW OVERLAP IS THE KEY
    let n = 2048;
    let overlap = 2;
    let mut sample_buffer = Vec::with_capacity(n);

    // Coherent gain of Hann Window
    let c_gain = FiniteF32::try_new(0.5).expect("We know 0.5 is finite");
    let two = FiniteF32::try_new(2.0).expect("We know 2.0 is finite");

    let analyzer_tx_handle = analyzer_tx.clone();

    let (cruncher_tx, cruncher_rx) = mpsc::channel::<Vec<f32>>();

    thread::spawn(move || {
        loop {
            let result = &cruncher_rx.recv();

            let windowed_samples = match result {
                Ok(packet) => {
                    // hann_window(packet)
                    packet
                },
                Err(err) => {
                    eprintln!("Error with analyzer -> cruncher channel: {err}");
                    break
                },
            };

            let spectrum = samples_fft_to_spectrum(
                &windowed_samples,
                config.sample_rate,
                spectrum_analyzer::FrequencyLimit::All,
                None
            ).unwrap();

            let n = spectrum.samples_len() as f32;
            let bins = create_bins(to_decibels(spectrum, n, c_gain, two));

            let _ = analyzer_tx_handle.send(to_ui_scale(bin_maximums(bins)));
        }
    });

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            for chunk in data.chunks_exact(channels) {
                let mono_sample = chunk.iter().sum::<f32>() / channels as f32;
                sample_buffer.push(mono_sample);

                if sample_buffer.len() == n {

                    let packet: Vec<f32> = hann_window(&sample_buffer);
                    _ = cruncher_tx.send(packet);

                    if overlap == 1 {
                        sample_buffer.clear();
                    } else {
                        let drop_size = n as usize / overlap;
                        sample_buffer.drain(0..drop_size);
                    }
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

fn to_decibels(
    spectrum: FrequencySpectrum,
    n: f32,
    c_gain: FiniteF32,
    two: FiniteF32,
) -> Vec<(NonNegF32, FiniteF32)> {
    const DB_FLOOR: f32 = 1e-5;
    let mut data = spectrum.data().to_owned();
    for (idx, datum) in &mut data.iter_mut().enumerate() {
        let temp = if n.abs() < 0.001 { datum.1.into() } else { datum.1 / n };
        if idx == 0 || idx == (spectrum.data().len() - 1) {
            // The DC and Nyquist bins don't get multiplied by 2, but still get divided by
            // the coherent gain of the window (0.5 for Hann)
            datum.1 = (20.0 * libm::log10f((temp / c_gain).max(DB_FLOOR))).into();
        } else {
            // Every other bin gets multiplied by 2 because this is a single sided spectrum
            // and then divided by the coherent gain of the window (0.5 for Hann)
            datum.1 = (20.0 * libm::log10f((temp / c_gain * two).max(DB_FLOOR))).into();
        }
    }
    
    return data
}

// Acceleracers bins: 31(?unsure) 83(??really unsure) 125 250 1k 2k(?unsure) 4k 8k 10k
fn create_bins(spectrum: Vec<(NonNegF32, FiniteF32)>) -> Vec<((f32, f32), Vec<FiniteF32>)> {
    let mut bins = vec![((0_f32, 20_f32), Vec::new()), ((20_f32, 30_f32), Vec::new()),
                        ((30_f32, 60_f32), Vec::new()), ((60_f32, 120_f32), Vec::new()),
                        ((120_f32, 280_f32), Vec::new()), ((280_f32, 700_f32), Vec::new()),
                        ((700_f32, 1500_f32), Vec::new()), ((1500_f32, 3000_f32), Vec::new()),
                        ((3000_f32, 7000_f32), Vec::new()), ((7000_f32, 15000_f32), Vec::new()),
                        ((15000_f32, 99999_f32), Vec::new())];
    
    for datum in spectrum.iter() {
        for bin in &mut bins {
            if datum.0 >= bin.0.0 && datum.0 < bin.0.1 {
                bin.1.push(datum.1);
            }
        }
    }

    return bins
}

// TODO see if there is a better way to represent "loudness" across a bin
fn bin_maximums (bins: Vec<((f32, f32), Vec<FiniteF32>)>) -> Vec<FiniteF32> {
    let mut maximums = Vec::new();
    for bin in bins {
        maximums.push(bin.1.into_iter().max().unwrap_or(0.0_f32.into()));
    }
    return maximums
}

fn to_ui_scale (mut bins: Vec<FiniteF32>) -> Vec<FiniteF32> {
    // Decibel values are -100 to 0
    // Divide by 100 to get -1 to 0, add 1 to get 0 to 1
    for bin in &mut bins {
        *bin = (*bin / 100.0 + 1.0).into();
    }
    return bins
}