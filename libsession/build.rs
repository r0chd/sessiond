use std::io;

fn main() -> io::Result<()> {
    #[cfg(feature = "sessiond")]
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT)
        .compile(&["../protocols/session_core_v1.xml"])?;

    Ok(())
}
