use crate::session_power_v1::session_power_manager;
use crate::{ecs, session, session_power_v1};
use std::process;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    Reboot,
    Poweroff,
    Suspend,
    Hibernate,
}

impl PowerAction {
    const fn name(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Poweroff => "poweroff",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Poweroff => "power off",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PowerError {
    ActiveSessions(String),
    Io(String),
}

impl hyprwire::Handler<session_power_v1::SessionPowerV1> for crate::Sessiond {
    fn bind(&mut self, object: session_power_manager::SessionPowerManager) {
        object.send_can_control(u32::from(can_control_power(
            &self.world,
            object.client().creds(),
        )));
        self.world
            .resource_mut::<ecs::PowerManagers>()
            .0
            .push(object);
    }
}

pub fn notify_power_control(world: &bevy_ecs::world::World) {
    let managers = world.resource::<ecs::PowerManagers>();
    for object in &managers.0 {
        object.send_can_control(u32::from(can_control_power(world, object.client().creds())));
    }
}

impl hyprwire::Dispatch<session_power_manager::SessionPowerManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_power_manager::SessionPowerManager,
        event: <session_power_manager::SessionPowerManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_power_manager::Event::Reboot => {
                let client = object.client();
                send_power_result(
                    object,
                    self.handle_power_action(PowerAction::Reboot, client.creds()),
                );
            }
            session_power_manager::Event::Poweroff => {
                let client = object.client();
                send_power_result(
                    object,
                    self.handle_power_action(PowerAction::Poweroff, client.creds()),
                );
            }
            session_power_manager::Event::Suspend => {
                let client = object.client();
                send_power_result(
                    object,
                    self.handle_power_action(PowerAction::Suspend, client.creds()),
                );
            }
            session_power_manager::Event::Hibernate => {
                let client = object.client();
                send_power_result(
                    object,
                    self.handle_power_action(PowerAction::Hibernate, client.creds()),
                );
            }
            session_power_manager::Event::Destroy => {
                self.world
                    .resource_mut::<ecs::PowerManagers>()
                    .0
                    .retain(|manager| manager != object);
            }
        }
    }
}

fn send_power_result(
    object: &session_power_manager::SessionPowerManager,
    result: Result<(), PowerError>,
) {
    match result {
        Ok(()) => object.send_done(),
        Err(PowerError::ActiveSessions(message)) => {
            object.send_error(session_power_v1::ErrorCode::PermissionDenied, message);
        }
        Err(PowerError::Io(message)) => {
            object.send_error(session_power_v1::ErrorCode::Io, message);
        }
    }
}

pub fn can_control_power(world: &bevy_ecs::world::World, creds: &hyprwire::Creds) -> bool {
    world
        .resource::<ecs::AccessRules>()
        .management
        .allows(creds)
        || session::entities(world)
            .into_iter()
            .filter_map(|entity| {
                Some((
                    world.get::<ecs::Uid>(entity)?,
                    world.get::<ecs::SessionClass>(entity)?,
                ))
            })
            .all(|(uid, session_class)| {
                uid.0 == creds.uid.as_raw() || *session_class != ecs::SessionClass::User
            })
}

impl crate::Sessiond {
    fn run_power_command(command: &[String], action: &str) -> Result<(), String> {
        let Some((program, args)) = command.split_first() else {
            log::warn!("{action} command is not configured");
            return Err(format!("no {action} command configured"));
        };

        log::info!(
            program = program.as_str(),
            args:? = args;
            "running {action} command"
        );

        match process::Command::new(program).args(args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => {
                log::error!(program = program.as_str(), status = status.to_string().as_str(); "{action} command failed");
                Err(format!("{action} command {program:?} exited with {status}"))
            }
            Err(err) => {
                log::error!(program = program.as_str(), error = err.to_string().as_str(); "failed to start {action} command");
                Err(format!(
                    "failed to start {action} command {program:?}: {err}"
                ))
            }
        }
    }

    pub fn handle_power_action(
        &self,
        action: PowerAction,
        creds: &hyprwire::Creds,
    ) -> Result<(), PowerError> {
        let foreign_session_count = session::entities(&self.world)
            .into_iter()
            .filter_map(|entity| {
                Some((
                    self.world.get::<ecs::Uid>(entity)?,
                    self.world.get::<ecs::SessionClass>(entity)?,
                ))
            })
            .filter(|(uid, session_class)| {
                uid.0 != creds.uid.as_raw() && **session_class == ecs::SessionClass::User
            })
            .count();

        if !can_control_power(&self.world, creds) {
            log::warn!(
                uid = creds.uid.as_raw(),
                pid = creds.pid.as_raw_pid(),
                foreign_session_count,
                action = action.name();
                "power action denied: multiple sessions active"
            );
            return Err(PowerError::ActiveSessions(format!(
                "cannot {} while multiple sessions are active (root privileges required)",
                action.display_name()
            )));
        }

        log::info!(
            uid = creds.uid.as_raw(),
            pid = creds.pid.as_raw_pid(),
            foreign_session_count,
            action = action.name();
            "power action accepted"
        );

        rustix::fs::sync();

        let config = &self.world.resource::<ecs::DaemonConfig>().0.power;
        let command = match action {
            PowerAction::Reboot => &config.reboot,
            PowerAction::Poweroff => &config.poweroff,
            PowerAction::Suspend => &config.suspend,
            PowerAction::Hibernate => &config.hibernate,
        };

        if let Err(err) = Self::run_power_command(command, action.name()) {
            return Err(PowerError::Io(err));
        }

        Ok(())
    }
}
