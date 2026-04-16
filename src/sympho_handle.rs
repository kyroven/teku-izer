use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::errors::Error as SymphError;

pub fn run_media(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    
    let mut hint = Hint::new();

    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            hint.with_extension(ext_str);
        }
    }

    let src = std::fs::File::open(path.to_owned())?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let dec_opts: DecoderOptions = Default::default();

    match symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts) {
        Ok(format) => {
            let mut format = format.format;

            let track = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or("No recognized audio tracks")?;
            
            let codec_params = track.codec_params.clone();

            let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;
            let mut sample_buf = None;
            let sample_buf_handle = &mut sample_buf;

            let track_id = track.id;

            loop {
                let packet = match format.next_packet() {
                    Ok(packet) => packet,
                    Err(SymphError::ResetRequired) => {
                        println!("Error reading next packet: chained OGG stream. I don't know if this is common enough to warrant implementing or if it should just be considered unsupported");
                        break
                    },
                    Err(SymphError::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        break
                    },
                    Err(err) => {
                        return Err(err.into())
                    }
                };

                // "Consume any new metadata since last packet" (I still have no idea what this means)
                while !format.metadata().is_latest() {
                    // "Pop the old head of the metadata queue" (what?)
                    format.metadata().pop();

                    // "Consume the new metadata at the head of the metadata queue"
                    println!("consume new metadata");
                    // TODO figure out what the fuck that all means
                }

                if packet.track_id() != track_id {
                    continue
                }

                let decoded = match decoder.decode(&packet) {
                    Ok(d) => d,
                    // "The packet failed to decode due to an IO error, skip the packet."
                    Err(SymphError::IoError(_)) => continue,
                    // "The packet failed to decode due to invalid data, skip the packet."
                    Err(SymphError::DecodeError(_)) => continue,
                    // "An unrecoverable error occured, halt decoding."
                    Err(err) => return Err(err.into()),
                };

                if sample_buf_handle.is_none() {
                    let spec = *decoded.spec();
                    let num_frames = decoded.frames();

                    *sample_buf_handle = Some(SampleBuffer::<f32>::new(num_frames as u64, spec));
                }

                sample_buf_handle.as_mut().unwrap().copy_interleaved_ref(decoded);
                let channels = &codec_params.channels.map(|c| c.count()).ok_or("hmm");
                
                // let spec = *decoded.spec();
                // let num_frames = decoded.frames();

                // let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);

            }

            // let mut metadata = format.metadata;
            // if let Some(data) = metadata.get() {
            //     let media = data.current().unwrap();
            // }
        },
        Err(err) => {
            // Should never reach this because file types are verified before they get passed to
            // this function
            eprintln!("unsupported format: sympho_handle > play() > format");
            return Err(Box::new(err))
        },
    };

    Ok(())
}