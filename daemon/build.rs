fn main() {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::SERVER)
        .compile(&["../protocols/moxsession_core_v1.xml"])
        .unwrap();
}
