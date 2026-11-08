use std::{fs::File, io::BufReader};

use rodio::{Decoder, OutputStreamBuilder, Sink};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the file path from the first command-line argument.
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: play-aac <file.aac | file.m4a>")?;

    // Open the default audio output device.
    // The stream must stay alive while audio is playing.
    let stream = OutputStreamBuilder::open_default_stream()?;

    // Create a sink connected to the stream's mixer.
    let sink = Sink::connect_new(stream.mixer());

    // Decode the file (the format is detected automatically).
    let file = File::open(path)?;
    let source = Decoder::new(BufReader::new(file))?;

    // Queue the source and block until playback finishes.
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}
