use std::io;

fn main() -> io::Result<()> {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::ALL)
        .compile(&[
            "../protocols/session_management_v1.xml",
            "../protocols/session_core_v1.xml",
            "../protocols/seat_v1.xml",
        ])?;

    Ok(())
}
