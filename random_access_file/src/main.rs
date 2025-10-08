use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write, Read};

fn main() -> std::io::Result<()> {

    let path = "random_access.txt";

    match std::fs::remove_file(path) {
        Ok(_) => { },
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => { }
        Err(e) => {
            eprintln!("Error removing file '{}': {}", path, e);
        }
    }

    // open file
    //  File::open(filename)   : read only
    //  File::create(filename) : write only
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(path)?;

    // write content
    file.write_all(b"Hello, Rust  random access!")?;

    // Seek to the 7th byte (after "Hello, ")
    file.seek(SeekFrom::Start(7))?;

    // Write "world" at that position
    file.write_all(b"world")?;

    // Read the modified content
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    println!("{}", buffer); // Output: Hello, world random access!

    Ok(())
}