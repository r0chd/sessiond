fn main() {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::CLIENT)
        .compile(&["../protocols/moxsession_pam_v1.xml"])
        .unwrap();
}
