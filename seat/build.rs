use std::io;

fn main() -> io::Result<()> {
    println!("cargo::rustc-link-arg-cdylib=-Wl,-soname,libseat.so.1");

    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT)
        .compile(&["../protocols/seat_v1.xml"])?;

    Ok(())
}
