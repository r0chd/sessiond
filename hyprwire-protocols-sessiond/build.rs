use std::io;

fn main() -> io::Result<()> {
    let scanner = hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT | hyprwire_scanner::Targets::SERVER);
    scanner.compile(&[
        "protocols/session_core_v1.xml",
        "protocols/session_inhibit_v1.xml",
        "protocols/session_management_v1.xml",
        "protocols/session_management_v2.xml",
        "protocols/session_backlight_v1.xml",
    ])
}
