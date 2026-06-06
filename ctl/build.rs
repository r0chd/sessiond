fn main() {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT)
        .compile(&[
            "../protocols/session_management_v1.xml",
            "../protocols/session_core_v1.xml",
        ])
        .unwrap();
}
