fn main() {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::SERVER)
        .compile(&[
            "../protocols/moxsession_login_v1.xml",
            "../protocols/moxsession_info_v1.xml",
        ])
        .unwrap();
}
