use std::{error::Error, fs::File, path::Path, sync::mpsc, sync::Arc};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Fully decoded audio: interleaved f32 samples.
struct Decoded {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
}

fn decode_file(path: &str) -> Result<Decoded, Box<dyn Error>> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // give the probe a hint from the file extension (optional but helps).
    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    // pick the first track that has a real codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track found")?;
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = 0u32;
    let mut channels = 0usize;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // end of stream is reported as an UnexpectedEof I/O error.
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };

        // skip packets that belong to other tracks.
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                channels = spec.channels.count();

                // allocate the conversion buffer once; AAC frames have a fixed size.
                let buf = sbuf
                    .get_or_insert_with(|| SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
            // a corrupt packet is not fatal: skip it and keep going.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    if channels == 0 || samples.is_empty() {
        return Err("no audio decoded".into());
    }
    Ok(Decoded { samples, sample_rate, channels })
}

/// Build an output stream for sample type `T`.
/// Resamples (linear interpolation) and maps channels on the fly.
fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    audio: Arc<Decoded>,
    done_tx: mpsc::Sender<()>,
) -> Result<Stream, Box<dyn Error>>
where
    T: SizedSample + FromSample<f32>,
{
    let out_ch = config.channels as usize;
    let src_ch = audio.channels;
    let frames = audio.samples.len() / src_ch;
    // how many source frames to advance per output frame.
    let step = audio.sample_rate as f64 / config.sample_rate.0 as f64;

    let mut pos = 0.0f64; // position in source frames
    let mut notified = false;

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(out_ch) {
                let i = pos as usize;

                // past the end: output silence and tell main() once.
                if i + 1 >= frames {
                    for s in frame.iter_mut() {
                        *s = T::EQUILIBRIUM;
                    }
                    if !notified {
                        notified = true;
                        let _ = done_tx.send(());
                    }
                    continue;
                }

                let frac = (pos - i as f64) as f32;
                for (c, out) in frame.iter_mut().enumerate() {
                    // channel mapping: mono is duplicated to all outputs,
                    // extra output channels beyond the source are silent.
                    let sc = if src_ch == 1 {
                        0
                    } else if c < src_ch {
                        c
                    } else {
                        *out = T::EQUILIBRIUM;
                        continue;
                    };
                    let a = audio.samples[i * src_ch + sc];
                    let b = audio.samples[(i + 1) * src_ch + sc];
                    *out = T::from_sample(a + (b - a) * frac);
                }
                pos += step;
            }
        },
        |err| eprintln!("stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: play-aac <file.aac | file.m4a>")?;

    // decode everything up front.
    let audio = Arc::new(decode_file(&path)?);
    println!(
        "decoded: {} Hz, {} ch, {:.1} s",
        audio.sample_rate,
        audio.channels,
        audio.samples.len() as f64 / audio.channels as f64 / audio.sample_rate as f64
    );

    // open the default output device with its default config.
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device")?;
    let supported = device.default_output_config()?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();

    // build a stream matching the device's native sample format.
    let (tx, rx) = mpsc::channel();
    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, audio, tx)?,
        SampleFormat::I16 => build_stream::<i16>(&device, &config, audio, tx)?,
        SampleFormat::U16 => build_stream::<u16>(&device, &config, audio, tx)?,
        other => return Err(format!("unsupported sample format: {other:?}").into()),
    };

    // start playback and wait until the callback reports the end.
    stream.play()?;
    rx.recv()?;
    // let the last buffer drain before dropping the stream.
    std::thread::sleep(std::time::Duration::from_millis(300));

    Ok(())
}
