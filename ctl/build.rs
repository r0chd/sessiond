use std::io;

fn main() -> io::Result<()> {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT)
        .compile(&[
            "../protocols/session_management_v1.xml",
            "../protocols/session_management_v2.xml",
            "../protocols/session_core_v1.xml",
            "../protocols/session_inhibit_v1.xml",
            "../protocols/session_power_v1.xml",
            "../protocols/seat_core_v1.xml",
        ])?;

    Ok(())
}
