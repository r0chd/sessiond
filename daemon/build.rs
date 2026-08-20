use std::io;

fn main() -> io::Result<()> {
    hyprwire_scanner::configure()
        .with_targets(hyprwire_scanner::Targets::ALL)
        .type_attribute(
            ".session_core_v1.active_state",
            "#[derive(bevy_ecs::component::Component)]",
        )
        .type_attribute(
            ".session_core_v1.user_state",
            "#[derive(bevy_ecs::component::Component)]",
        )
        .compile(&[
            "../protocols/session_management_v1.xml",
            "../protocols/session_management_v2.xml",
            "../protocols/session_core_v1.xml",
            "../protocols/session_power_v1.xml",
        ])?;

    Ok(())
}
