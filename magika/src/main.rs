fn main() -> Result<(), Box<dyn std::error::Error>> {

    // A Magika session can be used multiple times across multiple threads.
    let mut magika = magika::Session::new()?;

    // Files can be identified from their path.
    let result = magika.identify_file_sync("Cargo.toml")?;

    println!("label: {}", result.info().label);
    Ok(())
}
