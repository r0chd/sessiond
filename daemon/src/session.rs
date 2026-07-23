use crate::session_core_v1::{session_core_manager, session_core_session};
use crate::session_management_v1::session_manager as session_manager_v1;
use crate::session_management_v2::{session_management_session, session_manager};
use crate::{
    config, ecs, seat, session_core_v1, session_management_v1, session_management_v2, user,
};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use rustix::system;
use std::collections::BTreeMap;
use std::{fs, num, time};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SessionId(u32);

impl SessionId {
    pub const fn as_raw(self) -> u32 {
        self.0
    }

    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    world: &mut world::World,
    session_id: SessionId,
    uid: u32,
    username: String,
    vt: Option<u32>,
    seat: Option<String>,
    session_seat: Option<entity::Entity>,
    leader_id: i32,
    remote_host: Option<String>,
    active_state: session_core_v1::ActiveState,
    session_object: Option<session_management_session::SessionManagementSession>,
) -> entity::Entity {
    ecs::ensure_protocol_objects(world);
    let mut cmd = world.spawn(ecs::SessionBundle {
        marker: ecs::Session,
        id: ecs::SessionId(session_id),
        uid: ecs::Uid(uid),
        username: ecs::Username(username),
        timestamp: ecs::Timestamp(time::SystemTime::now()),
        leader_id: ecs::LeaderId(leader_id),
        env: ecs::SessionEnv(BTreeMap::new()),
        active_state: ecs::ActiveState(active_state),
    });
    if let Some(vt) = vt.and_then(num::NonZeroU32::new) {
        cmd.insert(ecs::Vt(vt));
    }
    if let Some(remote_host) = remote_host {
        cmd.insert(ecs::RemoteHost(remote_host));
    } else {
        cmd.insert(ecs::IsLocal);
    }
    if let Some(name) = seat {
        cmd.insert(ecs::SeatName(name));
    }
    if let Some(entity) = session_seat {
        cmd.insert(ecs::SessionSeat(entity));
    }
    if let Some(session_object) = session_object {
        cmd.insert(ecs::SessionObject(session_object));
    }
    cmd.id()
}

pub fn entities(world: &world::World) -> impl Iterator<Item = entity::Entity> + '_ {
    world
        .iter_entities()
        .filter(bevy_ecs::world::EntityRef::contains::<ecs::Session>)
        .map(|entity| entity.id())
}

pub fn id(world: &world::World, entity: entity::Entity) -> Option<SessionId> {
    world.get::<ecs::SessionId>(entity).map(|session| session.0)
}

pub fn entity(world: &world::World, session_id: SessionId) -> Option<entity::Entity> {
    let mut sessions =
        world.try_query_filtered::<(Entity, &ecs::SessionId), With<ecs::Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, id)| (id.0 == session_id).then_some(entity))
}

pub fn entity_by_uid(world: &world::World, uid: u32) -> Option<entity::Entity> {
    let mut sessions = world.try_query_filtered::<(Entity, &ecs::Uid), With<ecs::Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, session_uid)| (session_uid.0 == uid).then_some(entity))
}

pub fn entities_by_uid(
    world: &world::World,
    uid: u32,
) -> impl Iterator<Item = entity::Entity> + '_ {
    world
        .iter_entities()
        .filter(move |entity| {
            entity.contains::<ecs::Session>()
                && entity
                    .get::<ecs::Uid>()
                    .is_some_and(|session_uid| session_uid.0 == uid)
        })
        .map(|entity| entity.id())
}

pub fn entities_by_seat_name<'a>(
    world: &'a world::World,
    name: &'a str,
) -> impl Iterator<Item = entity::Entity> + 'a {
    world
        .iter_entities()
        .filter(move |entity| {
            entity.contains::<ecs::Session>()
                && entity
                    .get::<ecs::SeatName>()
                    .is_some_and(|session_seat| session_seat.0 == name)
        })
        .map(|entity| entity.id())
}

pub fn entity_by_object(
    world: &world::World,
    object: &session_core_session::SessionCoreSession,
) -> Option<entity::Entity> {
    world
        .resource::<ecs::ProtocolObjects>()
        .sessions
        .iter()
        .find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|session_object| session_object == object)
                .then_some(*entity)
        })
}

pub fn entity_by_management_object(
    world: &world::World,
    object: &session_management_session::SessionManagementSession,
) -> Option<entity::Entity> {
    let mut sessions =
        world.try_query_filtered::<(Entity, &ecs::SessionObject), With<ecs::Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, session_object)| (session_object.0 == *object).then_some(entity))
}

pub fn can_modify(
    world: &world::World,
    entity: entity::Entity,
    creds: &rustix::net::UCred,
) -> bool {
    let session_uid = world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
    creds.uid.is_root() || creds.uid.as_raw() == session_uid
}

pub fn session_envs_by_uid(
    world: &world::World,
    uid: u32,
    exclude: Option<entity::Entity>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for entity in entities_by_uid(world, uid) {
        if Some(entity) == exclude {
            continue;
        }
        if let Some(session_env) = world.get::<ecs::SessionEnv>(entity) {
            env.extend(session_env.0.clone());
        }
    }
    env
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = entity_by_object(&self.world, object) else {
            return;
        };
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        if matches!(event, session_core_session::Event::Destroy) {
            if let Some(objects) = self
                .world
                .resource_mut::<ecs::ProtocolObjects>()
                .sessions
                .get_mut(&entity)
            {
                objects.retain(|session_object| session_object != object);
            }
            return;
        }

        let Some(client) = object.client() else {
            return;
        };
        if !can_modify(&self.world, entity, client.creds()) {
            object.send_error(
                session_core_v1::ErrorCode::PermissionDenied,
                "permission denied",
            );
            return;
        }

        match event {
            session_core_session::Event::Terminate => {
                self.terminate_session(entity);
                user::sync_all(&mut self.world);
            }
            session_core_session::Event::Kill => {
                self.kill_session(entity);
                user::sync_all(&mut self.world);
            }
            session_core_session::Event::Lock => {
                log::info!(session_id = session_id.as_raw(); "lock requested");
                if let Some(objects) = self
                    .world
                    .resource::<ecs::ProtocolObjects>()
                    .sessions
                    .get(&entity)
                {
                    for object in objects {
                        object.send_locked();
                        object.send_ok();
                    }
                }
            }
            session_core_session::Event::Unlock => {
                log::info!(session_id = session_id.as_raw(); "unlock requested");
                if let Some(objects) = self
                    .world
                    .resource::<ecs::ProtocolObjects>()
                    .sessions
                    .get(&entity)
                {
                    for object in objects {
                        object.send_unlocked();
                        object.send_ok();
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl crate::Sessiond {
    pub fn kill_session(&mut self, entity: entity::Entity) {
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        log::info!(session_id = session_id.as_raw(); "kill requested");

        set_state(
            &mut self.world,
            session_id,
            entity,
            session_core_v1::ActiveState::Closing,
        );
        if let Err(err) = self.cgroup_manager.kill_session(session_id) {
            log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "kill failed");
        }
        #[cfg(feature = "consolekit")]
        self.sync_consolekit_session(session_id, entity);

        self.finish_session_destroy(session_id);
    }

    pub fn terminate_session(&mut self, entity: entity::Entity) {
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        log::info!(session_id = session_id.as_raw(); "terminate requested");

        set_state(
            &mut self.world,
            session_id,
            entity,
            session_core_v1::ActiveState::Closing,
        );
        if let Err(err) = self.cgroup_manager.terminate_session(session_id) {
            log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
        }
        #[cfg(feature = "consolekit")]
        self.sync_consolekit_session(session_id, entity);

        self.finish_session_destroy(session_id);
    }

    fn finish_session_destroy(&mut self, session_id: SessionId) {
        let Some(entity) = entity(&self.world, session_id) else {
            log::warn!(session_id = session_id.as_raw(); "destroy requested for unknown session");
            return;
        };

        let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
        let username = self
            .world
            .get::<ecs::Username>(entity)
            .map_or("", |username| username.0.as_str());
        log::info!(session_id = session_id.as_raw(), uid, username; "session destroyed");

        if let Some(objects) = self
            .world
            .resource::<ecs::ProtocolObjects>()
            .sessions
            .get(&entity)
        {
            for object in objects {
                object.send_destroyed();
            }
        }
        let _ = self.remove_session(session_id);
        user::sync_all(&mut self.world);

        self.cleanup_user_runtime_dir(uid);
    }

    pub(crate) fn cleanup_user_runtime_dir(&self, uid: u32) {
        let Some(user) = uzers::get_user_by_uid(uid) else {
            return;
        };

        if !self.cgroup_manager.has_uid_sessions(uid)
            && !user::is_lingering(&self.world, uid)
            && entities_by_uid(&self.world, uid).next().is_none()
            && let Err(e) = user::remove_runtime_dir(
                &self
                    .user_config(&user.name().to_string_lossy())
                    .runtime_dir(uid),
            )
        {
            log::error!(uid, error = e.to_string().as_str(); "failed to remove runtime dir");
        }
    }
}

pub fn add_object(
    world: &mut world::World,
    session_id: SessionId,
    entity: entity::Entity,
    object: session_core_session::SessionCoreSession,
) {
    log::debug!(session_id = session_id.as_raw(), uid = object.client().map_or(0, |c| c.creds().uid.as_raw()); "session info requested");

    object.send_session_id(session_id.as_raw());
    if let Some(uid) = world.get::<ecs::Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<ecs::Username>(entity) {
        object.send_username(&username.0);
    }
    if let Some(vt) = world.get::<ecs::Vt>(entity).map(|vt| vt.0.get()) {
        object.send_vt(vt);
    }
    if let Some(active_state) = world.get::<ecs::ActiveState>(entity) {
        object.send_active(active_state.0);
    }
    if let Some(client) = object.client() {
        object.send_can_modify(u32::from(can_modify(world, entity, client.creds())));
    }
    if let Some(seat) = world.get::<ecs::SeatName>(entity) {
        object.send_seat(&seat.0);
    }
    if let Some(leader_id) = world.get::<ecs::LeaderId>(entity) {
        object.send_leader(leader_id.0.cast_unsigned());
    }
    if let Some(timestamp) = world.get::<ecs::Timestamp>(entity) {
        let timestamp_secs = timestamp
            .0
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
        object.send_timestamp(timestamp);
    }
    if world.get::<ecs::IsLocal>(entity).is_some() {
        object.send_is_local(1);
    } else {
        object.send_is_local(0);
    }
    if let Some(remote_host) = world.get::<ecs::RemoteHost>(entity) {
        object.send_remote_host(&remote_host.0);
    }
    object.send_done();

    ecs::ensure_protocol_objects(world);
    world
        .resource_mut::<ecs::ProtocolObjects>()
        .sessions
        .entry(entity)
        .or_default()
        .push(object);
}

pub fn sync_vt_session_states(world: &mut world::World) {
    seat::refresh_current_vts(world);
    seat::sync_states(world);
    user::sync_all(world);
}

pub fn set_state(
    world: &mut world::World,
    session_id: SessionId,
    entity: entity::Entity,
    state: session_core_v1::ActiveState,
) {
    let vt = world
        .get::<ecs::Vt>(entity)
        .map(|vt| vt.0.get())
        .unwrap_or_default();
    log::info!(session_id = session_id.as_raw(), vt; "session state changed");
    if let Some(mut active_state) = world.get_mut::<ecs::ActiveState>(entity) {
        active_state.0 = state;
    }
    if let Some(objects) = world
        .get_resource::<ecs::ProtocolObjects>()
        .and_then(|objects| objects.sessions.get(&entity))
    {
        for object in objects {
            object.send_active(state);
        }
    }
}

impl session_management_v1::SessionManagementV1Handler for crate::Sessiond {}

impl session_management_v2::SessionManagementV2Handler for crate::Sessiond {}

impl session_core_v1::SessionCoreV1Handler for crate::Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        sync_vt_session_states(&mut self.world);

        entities(&self.world)
            .filter_map(|entity| id(&self.world, entity))
            .for_each(|session_id| {
                object.send_session_added(session_id.as_raw());
            });
        if let Some(client) = object.client() {
            object.send_can_control_power(u32::from(self.can_control_power(client.creds())));
        }
        object.send_done();
        self.world
            .resource_mut::<ecs::SessionInfoManagers>()
            .0
            .push(object);
    }
}

impl crate::Sessiond {
    // Allows to manage power commands (power off, reboot) only
    // if no other sessions are active or user has root permissions
    fn can_control_power(&self, creds: &rustix::net::UCred) -> bool {
        self.world
            .resource::<ecs::AccessRules>()
            .management
            .allows(creds)
            || entities(&self.world)
                .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                .all(|session| session.0 == creds.uid.as_raw())
    }

    pub fn user_config(&self, username: &str) -> config::UserConfig {
        self.world
            .resource::<ecs::DaemonConfig>()
            .0
            .users
            .get(username)
            .cloned()
            .unwrap_or_default()
    }
}

impl hyprwire::Dispatch<session_manager::SessionManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_manager::SessionManager,
        event: <session_manager::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        if !self
            .world
            .resource::<ecs::AccessRules>()
            .management
            .allows(client.creds())
        {
            log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
            return;
        }

        match event {
            session_manager::Event::Destroy => {}
            session_manager::Event::CreateSession {
                session_management_session,
                uid,
            } => {
                let session_id = self.world.resource_mut::<ecs::NextSessionId>().take();
                let username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(); "session created");

                let leader_id = client.creds().pid.as_raw_pid();

                if let Err(e) = self
                    .cgroup_manager
                    .create_session_cgroup(uid, session_id, leader_id)
                {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                seat::refresh_current_vts(&mut self.world);

                user::create_runtime_dir(uid, &self.user_config(&username).runtime_dir(uid));

                let _session_entity = spawn(
                    &mut self.world,
                    session_id,
                    uid,
                    username,
                    None,
                    None,
                    None,
                    leader_id,
                    None,
                    session_core_v1::ActiveState::Opening,
                    Some(session_management_session),
                );

                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, _session_entity);
                user::sync_all(&mut self.world);
            }
        }
    }
}

impl hyprwire::Dispatch<session_manager_v1::SessionManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_manager_v1::SessionManager,
        event: <session_manager_v1::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        if !self
            .world
            .resource::<ecs::AccessRules>()
            .management
            .allows(client.creds())
        {
            log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
            return;
        }

        match event {
            session_manager_v1::Event::CreateSession {
                uid,
                vt,
                is_local,
                remote_host,
            } => {
                let session_id = self.world.resource_mut::<ecs::NextSessionId>().take();
                let username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(), vt, is_local, remote_host = remote_host.as_str(); "session created");

                if let Err(e) = self.cgroup_manager.create_session_cgroup(
                    uid,
                    session_id,
                    client.creds().pid.as_raw_pid(),
                ) {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                seat::refresh_current_vts(&mut self.world);

                let is_local = is_local != 0;
                let session_vt = num::NonZeroU32::new(vt);
                let mut session_seat_entity = if session_vt.is_some() {
                    seat::first(&self.world)
                } else {
                    None
                };
                let mut session_seat = session_seat_entity.and_then(|seat_entity| {
                    self.world
                        .get::<ecs::SeatName>(seat_entity)
                        .map(|seat| seat.0.clone())
                        .filter(|seat| !seat.is_empty())
                });
                let mut active_state = session_core_v1::ActiveState::Online;

                user::create_runtime_dir(uid, &self.user_config(&username).runtime_dir(uid));

                if let Some((seat_entity, seat)) =
                    seat::entities(&self.world).find_map(|seat_entity| {
                        let mode = self.world.get::<ecs::SeatMode>(seat_entity)?;
                        if mode.0 != ecs::SeatKind::VtBound {
                            return None;
                        }
                        let current_vt = self.world.get::<ecs::CurrentVt>(seat_entity)?.0;
                        (Some(current_vt) == session_vt && is_local).then(|| {
                            self.world
                                .get::<ecs::SeatName>(seat_entity)
                                .map(|seat| (seat_entity, seat.0.clone()))
                        })?
                    })
                {
                    session_seat_entity = Some(seat_entity);
                    session_seat = Some(seat);
                    active_state = session_core_v1::ActiveState::Active;
                }

                let mut env = self.user_config(&username).environment(
                    uid,
                    session_id.as_raw(),
                    session_seat.as_deref(),
                );
                env.extend(session_envs_by_uid(&self.world, uid, None));

                let session_entity = spawn(
                    &mut self.world,
                    session_id,
                    uid,
                    username,
                    session_vt.map(num::NonZeroU32::get),
                    session_seat.clone(),
                    session_seat_entity,
                    client.creds().pid.as_raw_pid(),
                    if is_local { None } else { Some(remote_host) },
                    active_state,
                    None,
                );

                let pending_env = self
                    .world
                    .resource_mut::<ecs::PendingSessionEnvs>()
                    .0
                    .remove(object);
                if let Some(pending_env) = pending_env
                    && let Some(mut session_env) =
                        self.world.get_mut::<ecs::SessionEnv>(session_entity)
                {
                    session_env.0 = pending_env;
                }

                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, session_entity);
                user::sync_all(&mut self.world);

                object.send_session_id(session_id.as_raw());
                object.send_seat(session_seat.as_deref().unwrap_or_default());
                for (name, value) in env {
                    object.send_env(name.as_str(), value.as_str());
                }
                object.send_done();

                for manager in &self.world.resource::<ecs::SessionInfoManagers>().0 {
                    manager.send_session_added(session_id.as_raw());
                }
            }
            session_manager_v1::Event::Env { name, value } => {
                self.world
                    .resource_mut::<ecs::PendingSessionEnvs>()
                    .0
                    .entry(object.clone())
                    .or_default()
                    .insert(name, value);
            }
            session_manager_v1::Event::DestroySession { session_id } => {
                if let Some(entity) = entity(&self.world, SessionId::from_raw(session_id)) {
                    self.terminate_session(entity);
                }
            }
            session_manager_v1::Event::Destroy => {
                self.world
                    .resource_mut::<ecs::PendingSessionEnvs>()
                    .0
                    .remove(object);
            }
        }
    }
}

impl hyprwire::Dispatch<session_management_session::SessionManagementSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_management_session::SessionManagementSession,
        event: <session_management_session::SessionManagementSession as hyprwire::Object>::Event<
            '_,
        >,
    ) {
        let Some(entity) = entity_by_management_object(&self.world, object) else {
            return;
        };
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        match event {
            session_management_session::Event::Vt { vt } => {
                if let Some(vt) = num::NonZeroU32::new(vt) {
                    self.world.entity_mut(entity).insert(ecs::Vt(vt));
                }
            }
            session_management_session::Event::RemoteHost { remote_host } => {
                self.world
                    .entity_mut(entity)
                    .insert(ecs::RemoteHost(remote_host));
                self.world.entity_mut(entity).remove::<ecs::IsLocal>();
            }
            session_management_session::Event::Done => {
                let uid = self.world.get::<ecs::Uid>(entity).map(|uid| uid.0);
                let username = self
                    .world
                    .get::<ecs::Username>(entity)
                    .map(|username| username.0.clone());
                let session_vt = self.world.get::<ecs::Vt>(entity).map(|vt| vt.0);
                let is_local = self.world.get::<ecs::IsLocal>(entity).is_some();
                seat::refresh_current_vts(&mut self.world);

                let mut session_seat_entity = if session_vt.is_some() {
                    seat::first(&self.world)
                } else {
                    None
                };
                let mut session_seat = session_seat_entity.and_then(|seat_entity| {
                    self.world
                        .get::<ecs::SeatName>(seat_entity)
                        .map(|seat| seat.0.clone())
                        .filter(|seat| !seat.is_empty())
                });

                let active_state = if let Some((seat_entity, seat)) = seat::entities(&self.world)
                    .find_map(|seat_entity| {
                        let mode = self.world.get::<ecs::SeatMode>(seat_entity)?;
                        if mode.0 != ecs::SeatKind::VtBound {
                            return None;
                        }
                        let current_vt = self.world.get::<ecs::CurrentVt>(seat_entity)?.0;
                        (Some(current_vt) == session_vt && is_local).then(|| {
                            self.world
                                .get::<ecs::SeatName>(seat_entity)
                                .map(|seat| (seat_entity, seat.0.clone()))
                        })?
                    }) {
                    session_seat_entity = Some(seat_entity);
                    session_seat = Some(seat);
                    session_core_v1::ActiveState::Active
                } else {
                    session_core_v1::ActiveState::Online
                };

                {
                    let mut session = self.world.entity_mut(entity);
                    if let Some(seat) = session_seat.as_ref() {
                        session.insert(ecs::SeatName(seat.clone()));
                    } else {
                        session.remove::<ecs::SeatName>();
                    }
                    if let Some(seat_entity) = session_seat_entity {
                        session.insert(ecs::SessionSeat(seat_entity));
                    } else {
                        session.remove::<ecs::SessionSeat>();
                    }
                }

                set_state(&mut self.world, session_id, entity, active_state);
                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, entity);
                user::sync_all(&mut self.world);

                object.send_session_id(session_id.as_raw());
                object.send_seat(session_seat.as_deref().unwrap_or_default());
                if let (Some(uid), Some(username)) = (uid, username) {
                    let mut env = self.user_config(&username).environment(
                        uid,
                        session_id.as_raw(),
                        session_seat.as_deref(),
                    );
                    env.extend(session_envs_by_uid(&self.world, uid, Some(entity)));

                    for (name, value) in env {
                        object.send_env(name.as_str(), value.as_str());
                    }
                }
                object.send_done();

                for manager in &self.world.resource::<ecs::SessionInfoManagers>().0 {
                    manager.send_session_added(session_id.as_raw());
                }
            }
            session_management_session::Event::Env { name, value } => {
                if let Some(mut env) = self.world.get_mut::<ecs::SessionEnv>(entity) {
                    env.0.insert(name, value);
                } else {
                    self.world
                        .entity_mut(entity)
                        .insert(ecs::SessionEnv(BTreeMap::from([(name, value)])));
                }
            }
            session_management_session::Event::Destroy => {
                self.terminate_session(entity);
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::GetSession {
                session_core_session,
                session_id,
            } => {
                sync_vt_session_states(&mut self.world);

                let session_id = SessionId::from_raw(session_id);
                if let Some(entity) = entity(&self.world, session_id) {
                    add_object(&mut self.world, session_id, entity, session_core_session);
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByPid {
                session_core_session,
                pid,
            } => {
                sync_vt_session_states(&mut self.world);

                let Ok(Some(session_id)) = self.cgroup_manager.find_session_by_pid(pid) else {
                    log::warn!(pid; "get_session_by_pid: no session found for pid");
                    session_core_session.send_destroyed();
                    return;
                };

                if let Some(entity) = entity(&self.world, session_id) {
                    add_object(&mut self.world, session_id, entity, session_core_session);
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session_by_pid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByUid {
                session_core_session,
                uid,
            } => {
                sync_vt_session_states(&mut self.world);

                if let Some(entity) = entity_by_uid(&self.world, uid)
                    && let Some(session_id) = id(&self.world, entity)
                {
                    add_object(&mut self.world, session_id, entity, session_core_session);
                    return;
                }

                log::warn!(uid; "get_session_by_uid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetUser {
                session_core_user,
                uid,
            } => {
                let entity = user::entity_by_uid(&self.world, uid)
                    .or_else(|| user::create(&mut self.world, uid, false));
                let Some(entity) = entity else {
                    log::warn!(uid; "get_user requested for unknown user");
                    session_core_user.send_not_found();
                    return;
                };

                let management_access = self.world.resource::<ecs::AccessRules>().management;
                user::add_object(
                    &mut self.world,
                    entity,
                    session_core_user,
                    management_access,
                );
            }
            session_core_manager::Event::Reboot => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user reboot only if there are no other user sessions active
                let foreign_session_count = entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !self.can_control_power(client.creds()) {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "reboot denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot reboot while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "reboot accepted");

                for session_id in entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::SessionId>(entity))
                {
                    if let Err(err) = self.cgroup_manager.terminate_session(session_id.0) {
                        log::error!(session_id = session_id.0.as_raw(), error = err.to_string().as_str(); "failed to terminate session before reboot");
                        object.send_error(
                            session_core_v1::ErrorCode::Io,
                            format!(
                                "failed to terminate session {}: {err}",
                                session_id.0.as_raw()
                            ),
                        );
                        return;
                    }
                }

                rustix::fs::sync();

                if let Err(err) = system::reboot(system::RebootCommand::Restart) {
                    log::error!(error = err.to_string().as_str(); "reboot syscall failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to reboot: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Poweroff => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user poweroff only if there are no other user sessions active
                let foreign_session_count = entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !self.can_control_power(client.creds()) {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "poweroff denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot power off while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "poweroff accepted");

                for session_id in entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::SessionId>(entity))
                {
                    if let Err(err) = self.cgroup_manager.terminate_session(session_id.0) {
                        log::error!(session_id = session_id.0.as_raw(), error = err.to_string().as_str(); "failed to terminate session before poweroff");
                        object.send_error(
                            session_core_v1::ErrorCode::Io,
                            format!(
                                "failed to terminate session {}: {err}",
                                session_id.0.as_raw()
                            ),
                        );
                        return;
                    }
                }

                rustix::fs::sync();

                if let Err(err) = system::reboot(system::RebootCommand::PowerOff) {
                    log::error!(error = err.to_string().as_str(); "poweroff syscall failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to power off: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Suspend => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user suspend only if there are no other user sessions active
                let foreign_session_count = entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !self.can_control_power(client.creds()) {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "suspend denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot suspend while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "suspend accepted");

                rustix::fs::sync();

                if let Err(err) = fs::write("/sys/power/state", "mem") {
                    log::error!(error = err.to_string().as_str(); "suspend failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to suspend: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Hibernate => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user hibernate only if there are no other user sessions active
                let foreign_session_count = entities(&self.world)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !self.can_control_power(client.creds()) {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "hibernate denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot hibernate while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "hibernate accepted");

                rustix::fs::sync();

                if let Err(err) = fs::write("/sys/power/state", "disk") {
                    log::error!(error = err.to_string().as_str(); "hibernate failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to hibernate: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::GetSeat {
                session_core_seat,
                seat,
            } => {
                let Some(seat_entity) = seat::entity_by_name(&self.world, &seat) else {
                    session_core_seat.send_destroyed();
                    return;
                };
                let management_access = self.world.resource::<ecs::AccessRules>().management;
                seat::add_object(
                    &mut self.world,
                    seat_entity,
                    session_core_seat,
                    management_access,
                );
            }
            session_core_manager::Event::Destroy => {
                self.world
                    .resource_mut::<ecs::SessionInfoManagers>()
                    .0
                    .retain(|m| m != object);
            }
        }
    }
}
