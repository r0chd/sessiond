#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::session_power_v1::session_power_manager;
use crate::{config, inhibit, session, session_inhibit_v1, session_power_v1};
use bevy_ecs::component;
use bevy_ecs::prelude::*;
use std::process;

impl Component for session_power_manager::SessionPowerManager {
    const STORAGE_TYPE: component::StorageType = component::StorageType::Table;
    type Mutability = component::Mutable;
}

#[derive(Component)]
pub struct PowerManager;

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

pub fn initialize_power_manager_permissions(
    managers: Query<
        &session_power_manager::SessionPowerManager,
        (With<PowerManager>, Added<PowerManager>),
    >,
    sessions: Query<(&session::Uid, &session::SessionClass), With<session::Session>>,
) {
    for manager in &managers {
        let can_control = manager.client().creds().uid.is_root()
            || sessions
                .iter()
                .filter(|(_, class)| **class == session::SessionClass::User)
                .all(|(uid, _)| uid.0 == manager.client().creds().uid.as_raw());
        manager.send_can_control(u32::from(can_control));
    }
}

impl hyprwire::Handler<session_power_v1::SessionPowerV1> for crate::Sessiond {
    fn bind(&mut self, object: session_power_manager::SessionPowerManager) {
        self.world.spawn((PowerManager, object));
    }
}

pub fn notify_power_control(
    power_managers: Query<&session_power_manager::SessionPowerManager, With<PowerManager>>,
    changed_sessions: Query<
        (),
        (
            With<session::Session>,
            Or<(
                Added<session::Session>,
                Changed<session::Uid>,
                Changed<session::SessionClass>,
            )>,
        ),
    >,
    sessions: Query<(&session::Uid, &session::SessionClass), With<session::Session>>,
    mut removed_sessions: RemovedComponents<session::Session>,
) {
    if changed_sessions.is_empty() && removed_sessions.read().next().is_none() {
        return;
    }

    for object in &power_managers {
        let can_control = object.client().creds().uid.is_root()
            || sessions
                .iter()
                .filter(|(_, class)| **class == session::SessionClass::User)
                .all(|(uid, _)| uid.0 == object.client().creds().uid.as_raw());

        object.send_can_control(u32::from(can_control));
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
                self.world.write_message(PowerActionRequest {
                    action: PowerAction::Reboot,
                    creds: *client.creds(),
                    responder: PowerActionResponder::Manager(object.clone()),
                });
            }
            session_power_manager::Event::Poweroff => {
                let client = object.client();
                self.world.write_message(PowerActionRequest {
                    action: PowerAction::Poweroff,
                    creds: *client.creds(),
                    responder: PowerActionResponder::Manager(object.clone()),
                });
            }
            session_power_manager::Event::Suspend => {
                let client = object.client();
                self.world.write_message(PowerActionRequest {
                    action: PowerAction::Suspend,
                    creds: *client.creds(),
                    responder: PowerActionResponder::Manager(object.clone()),
                });
            }
            session_power_manager::Event::Hibernate => {
                let client = object.client();
                self.world.write_message(PowerActionRequest {
                    action: PowerAction::Hibernate,
                    creds: *client.creds(),
                    responder: PowerActionResponder::Manager(object.clone()),
                });
            }
            session_power_manager::Event::Destroy => {
                self.world.write_message(PowerManagerDestroy {
                    object: object.clone(),
                });
            }
        }
    }
}

#[derive(Message)]
pub struct PowerManagerDestroy {
    pub object: session_power_manager::SessionPowerManager,
}

pub fn handle_power_manager_control(
    mut actions: MessageReader<PowerManagerDestroy>,
    managers: Query<(Entity, &session_power_manager::SessionPowerManager), With<PowerManager>>,
    mut commands: Commands,
) {
    for action in actions.read() {
        if let Some((entity, _)) = managers
            .iter()
            .find(|(_, manager)| *manager == &action.object)
        {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Message)]
pub struct PowerActionRequest {
    pub action: PowerAction,
    pub creds: hyprwire::Creds,
    pub responder: PowerActionResponder,
}

pub enum PowerActionResponder {
    Manager(session_power_manager::SessionPowerManager),
    #[cfg(feature = "consolekit")]
    ConsoleKit(std::sync::mpsc::Sender<consolekit::PowerResponse>),
}

fn send_power_result(responder: &PowerActionResponder, result: Result<(), PowerError>) {
    match responder {
        PowerActionResponder::Manager(manager) => match result {
            Ok(()) => manager.send_done(),
            Err(PowerError::ActiveSessions(message)) => {
                manager.send_error(session_power_v1::ErrorCode::PermissionDenied, message);
            }
            Err(PowerError::Io(message)) => {
                manager.send_error(session_power_v1::ErrorCode::Io, message);
            }
        },
        #[cfg(feature = "consolekit")]
        PowerActionResponder::ConsoleKit(reply) => {
            let _ = reply.send(consolekit::PowerResponse::Done(result));
        }
    }
}

pub fn handle_power_action(
    mut requests: MessageReader<PowerActionRequest>,
    config: Res<config::DaemonConfig>,
    sessions: Query<(&session::Uid, &session::SessionClass), With<session::Session>>,
    shutdown_inhibitors: Query<
        (
            &session_inhibit_v1::Mode,
            &inhibit::Who,
            &inhibit::Why,
            &inhibit::Uid,
        ),
        With<inhibit::Shutdown>,
    >,
    sleep_inhibitors: Query<
        (
            &session_inhibit_v1::Mode,
            &inhibit::Who,
            &inhibit::Why,
            &inhibit::Uid,
        ),
        With<inhibit::Sleep>,
    >,
) {
    for request in requests.read() {
        let blocks = |(mode, _, _, owner): (
            &session_inhibit_v1::Mode,
            &inhibit::Who,
            &inhibit::Why,
            &inhibit::Uid,
        )| match mode {
            session_inhibit_v1::Mode::Block => true,
            session_inhibit_v1::Mode::BlockWeak => {
                !request.creds.uid.is_root() && owner.0 != request.creds.uid.as_raw()
            }
            session_inhibit_v1::Mode::Delay => false,
        };
        if match request.action {
            PowerAction::Reboot | PowerAction::Poweroff => shutdown_inhibitors.iter().any(blocks),
            PowerAction::Suspend | PowerAction::Hibernate => sleep_inhibitors.iter().any(blocks),
        } {
            send_power_result(
                &request.responder,
                Err(PowerError::ActiveSessions(format!(
                    "cannot {} while an active inhibitor blocks the operation",
                    request.action.display_name(),
                ))),
            );
            continue;
        }

        let foreign_session_count = sessions
            .iter()
            .filter(|(uid, class)| {
                uid.0 != request.creds.uid.as_raw() && **class == session::SessionClass::User
            })
            .count();

        if !request.creds.uid.is_root() && foreign_session_count != 0 {
            log::warn!(
                uid = request.creds.uid.as_raw(),
                pid = request.creds.pid.as_raw_pid(),
                foreign_session_count,
                action = request.action.name();
                "power action denied: multiple sessions active"
            );
            send_power_result(
                &request.responder,
                Err(PowerError::ActiveSessions(format!(
                    "cannot {} while multiple sessions are active (root privileges required)",
                    request.action.display_name()
                ))),
            );
            continue;
        }

        log::info!(
            uid = request.creds.uid.as_raw(),
            pid = request.creds.pid.as_raw_pid(),
            foreign_session_count,
            action = request.action.name();
            "power action accepted"
        );

        rustix::fs::sync();

        let command = match request.action {
            PowerAction::Reboot => &config.0.power.reboot,
            PowerAction::Poweroff => &config.0.power.poweroff,
            PowerAction::Suspend => &config.0.power.suspend,
            PowerAction::Hibernate => &config.0.power.hibernate,
        };

        if let Err(err) = run_power_command(command, request.action.name()) {
            send_power_result(&request.responder, Err(PowerError::Io(err)));
            continue;
        }

        send_power_result(&request.responder, Ok(()));
    }
}

#[cfg(feature = "consolekit")]
#[derive(Message)]
pub struct PowerCanRequest {
    pub action: PowerAction,
    pub creds: hyprwire::Creds,
    pub responder: std::sync::mpsc::Sender<consolekit::PowerResponse>,
}

#[cfg(feature = "consolekit")]
pub fn handle_power_can(
    mut requests: MessageReader<PowerCanRequest>,
    sessions: Query<(&session::Uid, &session::SessionClass), With<session::Session>>,
) {
    for request in requests.read() {
        let _ = request.action;
        let can = request.creds.uid.is_root()
            || sessions
                .iter()
                .filter(|(_, class)| **class == session::SessionClass::User)
                .all(|(uid, _)| uid.0 == request.creds.uid.as_raw());
        let _ = request.responder.send(consolekit::PowerResponse::Can(can));
    }
}

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
