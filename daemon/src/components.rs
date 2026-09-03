#[derive(bevy_ecs::component::Component)]
pub struct SessionInhibitor(
    pub crate::session_inhibit_v1::server::session_inhibitor::SessionInhibitor,
);
