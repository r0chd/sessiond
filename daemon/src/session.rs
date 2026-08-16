use crate::session_core_v1::{session_core_manager, session_core_session};
use crate::session_management_v1::session_manager as session_manager_v1;
use crate::session_management_v2::{session_management_session, session_manager};
use crate::{
    access, cgroup, config, protocol, session_core_v1, session_management_v1,
    session_management_v2, user, vt,
};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use hyprwire::server;
use std::collections::{BTreeMap, HashMap};
use std::{num, time};

#[derive(Resource)]
pub struct NextSessionId(pub SessionId);

impl NextSessionId {
    pub const fn take(&mut self) -> SessionId {
        let id = self.0;
        self.0 = self.0.next();
        id
    }
}

#[derive(Resource, Default)]
pub struct SessionInfoManagers(pub Vec<session_core_manager::SessionCoreManager>);

#[derive(Resource, Default)]
pub struct PendingSessionEnvs(
    pub HashMap<session_manager_v1::SessionManager, BTreeMap<String, String>>,
);

#[derive(Bundle)]
pub struct SessionBundle {
    pub marker: Session,
    pub id: SessionIdComponent,
    pub class: SessionClass,
    pub uid: Uid,
    pub username: Username,
    pub timestamp: Timestamp,
    pub leader_id: LeaderId,
    pub env: SessionEnv,
    pub active_state: ActiveState,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsLocal;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uid(pub u32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Username(pub String);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdComponent(pub SessionId);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vt(pub num::NonZeroU32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp(pub time::SystemTime);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderId(pub i32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RemoteHost(pub String);

#[derive(Component)]
pub struct SessionEnv(pub BTreeMap<String, String>);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionClass {
    #[default]
    User,
    Greeter,
    LockScreen,
    Background,
    BackgroundLight,
    Manager,
    ManagerEarly,
}

macro_rules! class_from {
    ($protocol:ty) => {
        impl From<$protocol> for SessionClass {
            fn from(class: $protocol) -> Self {
                match class {
                    <$protocol>::User => Self::User,
                    <$protocol>::Greeter => Self::Greeter,
                    <$protocol>::LockScreen => Self::LockScreen,
                    <$protocol>::Background => Self::Background,
                    <$protocol>::BackgroundLight => Self::BackgroundLight,
                    <$protocol>::Manager => Self::Manager,
                    <$protocol>::ManagerEarly => Self::ManagerEarly,
                }
            }
        }
    };
}
class_from!(session_management_v1::SessionClass);
class_from!(session_management_v2::SessionClass);

impl From<SessionClass> for session_core_v1::SessionClass {
    fn from(class: SessionClass) -> Self {
        match class {
            SessionClass::User => Self::User,
            SessionClass::Greeter => Self::Greeter,
            SessionClass::LockScreen => Self::LockScreen,
            SessionClass::Background => Self::Background,
            SessionClass::BackgroundLight => Self::BackgroundLight,
            SessionClass::Manager => Self::Manager,
            SessionClass::ManagerEarly => Self::ManagerEarly,
        }
    }
}

#[derive(Component)]
pub struct ActiveState(pub session_core_v1::ActiveState);

#[derive(Component)]
pub struct SessionObject(pub session_management_session::SessionManagementSession);

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
    class: SessionClass,
    uid: u32,
    username: String,
    vt: Option<u32>,
    leader_id: i32,
    remote_host: Option<String>,
    active_state: session_core_v1::ActiveState,
    session_object: Option<session_management_session::SessionManagementSession>,
) -> entity::Entity {
    let mut cmd = world.spawn(SessionBundle {
        marker: Session,
        id: SessionIdComponent(session_id),
        class,
        uid: Uid(uid),
        username: Username(username),
        timestamp: Timestamp(time::SystemTime::now()),
        leader_id: LeaderId(leader_id),
        env: SessionEnv(BTreeMap::new()),
        active_state: ActiveState(active_state),
    });
    if let Some(vt) = vt.and_then(num::NonZeroU32::new) {
        cmd.insert(Vt(vt));
    }
    if let Some(remote_host) = remote_host {
        cmd.insert(RemoteHost(remote_host));
    } else {
        cmd.insert(IsLocal);
    }
    if let Some(session_object) = session_object {
        cmd.insert(SessionObject(session_object));
    }
    cmd.id()
}

pub fn entities(world: &world::World) -> Vec<entity::Entity> {
    let Some(mut sessions) = world.try_query_filtered::<Entity, With<Session>>() else {
        return Vec::new();
    };
    sessions.iter(world).collect()
}

pub fn id(world: &world::World, entity: entity::Entity) -> Option<SessionId> {
    world
        .get::<SessionIdComponent>(entity)
        .map(|session| session.0)
}

pub fn entity(world: &world::World, session_id: SessionId) -> Option<entity::Entity> {
    let mut sessions =
        world.try_query_filtered::<(Entity, &SessionIdComponent), With<Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, id)| (id.0 == session_id).then_some(entity))
}

pub fn entity_by_uid(world: &world::World, uid: u32) -> Option<entity::Entity> {
    let mut sessions = world.try_query_filtered::<(Entity, &Uid), With<Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, session_uid)| (session_uid.0 == uid).then_some(entity))
}

pub fn entities_by_uid(world: &world::World, uid: u32) -> Vec<entity::Entity> {
    let Some(mut sessions) = world.try_query_filtered::<(Entity, &Uid), With<Session>>() else {
        return Vec::new();
    };
    sessions
        .iter(world)
        .filter_map(|(entity, session_uid)| (session_uid.0 == uid).then_some(entity))
        .collect()
}

pub fn entity_by_object(
    world: &world::World,
    object: &session_core_session::SessionCoreSession,
) -> Option<entity::Entity> {
    world
        .resource::<protocol::ProtocolObjects>()
        .session_entity_by_object(object)
}

pub fn entity_by_management_object(
    world: &world::World,
    object: &session_management_session::SessionManagementSession,
) -> Option<entity::Entity> {
    let mut sessions = world.try_query_filtered::<(Entity, &SessionObject), With<Session>>()?;
    sessions
        .iter(world)
        .find_map(|(entity, session_object)| (session_object.0 == *object).then_some(entity))
}

pub fn can_modify(world: &world::World, entity: entity::Entity, creds: &hyprwire::Creds) -> bool {
    let session_uid = world.get::<Uid>(entity).map_or(0, |uid| uid.0);
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
        if let Some(session_env) = world.get::<SessionEnv>(entity) {
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
            self.world
                .resource_mut::<protocol::ProtocolObjects>()
                .remove_session_object(entity, object);
            return;
        }

        let client = object.client();
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
            }
            session_core_session::Event::Kill => {
                self.kill_session(entity);
            }
            session_core_session::Event::Lock => {
                log::info!(session_id = session_id.as_raw(); "lock requested");
                if let Some(objects) = self
                    .world
                    .resource::<protocol::ProtocolObjects>()
                    .session_objects(entity)
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
                    .resource::<protocol::ProtocolObjects>()
                    .session_objects(entity)
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

        set_active_state(
            &mut self.world,
            entity,
            session_core_v1::ActiveState::Closing,
        );
        let Some(loop_handle) = self
            .world
            .get_non_send::<calloop::LoopHandle<'static, Self>>()
        else {
            return;
        };
        if let Err(err) = self
            .world
            .resource::<cgroup::CgroupManager>()
            .kill_session(loop_handle, session_id)
        {
            log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "kill failed");
        }
        self.finish_session_destroy(session_id);
    }

    pub fn terminate_session(&mut self, entity: entity::Entity) {
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        log::info!(session_id = session_id.as_raw(); "terminate requested");

        set_active_state(
            &mut self.world,
            entity,
            session_core_v1::ActiveState::Closing,
        );
        let Some(loop_handle) = self
            .world
            .get_non_send::<calloop::LoopHandle<'static, Self>>()
        else {
            return;
        };
        if let Err(err) = self
            .world
            .resource::<cgroup::CgroupManager>()
            .terminate_session(loop_handle, session_id)
        {
            log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
        }
        self.finish_session_destroy(session_id);
    }

    fn finish_session_destroy(&mut self, session_id: SessionId) {
        let Some(entity) = entity(&self.world, session_id) else {
            log::warn!(session_id = session_id.as_raw(); "destroy requested for unknown session");
            return;
        };

        let uid = self.world.get::<Uid>(entity).map_or(0, |uid| uid.0);
        let username = self
            .world
            .get::<Username>(entity)
            .map_or("", |username| username.0.as_str());
        log::info!(session_id = session_id.as_raw(), uid, username; "session destroyed");

        if let Some(objects) = self
            .world
            .resource::<protocol::ProtocolObjects>()
            .session_objects(entity)
        {
            for object in objects {
                object.send_destroyed();
            }
        }
        let _ = self.remove_session(session_id);

        self.cleanup_user_runtime_dir(uid);
    }

    pub(crate) fn cleanup_user_runtime_dir(&self, uid: u32) {
        let Some(user) = uzers::get_user_by_uid(uid) else {
            return;
        };

        if !self
            .world
            .resource::<cgroup::CgroupManager>()
            .has_uid_sessions(uid)
            && !user::is_lingering(&self.world, uid)
            && entities_by_uid(&self.world, uid).is_empty()
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
    log::debug!(session_id = session_id.as_raw(), uid = object.client().creds().uid.as_raw(); "session info requested");

    object.send_session_id(session_id.as_raw());
    if let Some(uid) = world.get::<Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<Username>(entity) {
        object.send_username(&username.0);
    }
    if let Some(class) = world.get::<SessionClass>(entity) {
        object.send_class((*class).into());
    }
    if let Some(vt) = world.get::<Vt>(entity).map(|vt| vt.0.get()) {
        object.send_vt(vt);
    }
    if let Some(active_state) = world.get::<ActiveState>(entity) {
        object.send_active(active_state.0);
    }
    object.send_can_modify(u32::from(can_modify(
        world,
        entity,
        object.client().creds(),
    )));
    if let Some(leader_id) = world.get::<LeaderId>(entity) {
        object.send_leader(leader_id.0.cast_unsigned());
    }
    if let Some(timestamp) = world.get::<Timestamp>(entity) {
        let timestamp_secs = timestamp
            .0
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
        object.send_timestamp(timestamp);
    }
    if world.get::<IsLocal>(entity).is_some() {
        object.send_is_local(1);
    } else {
        object.send_is_local(0);
    }
    if let Some(remote_host) = world.get::<RemoteHost>(entity) {
        object.send_remote_host(&remote_host.0);
    }
    object.send_done();

    world
        .resource_mut::<protocol::ProtocolObjects>()
        .add_session_object(entity, object);
}

pub fn sync_vt_session_states(world: &mut world::World) {
    vt::refresh_current_vts(world);
}

pub fn set_active_state(
    world: &mut world::World,
    entity: entity::Entity,
    state: session_core_v1::ActiveState,
) {
    let session_id = world
        .get::<SessionIdComponent>(entity)
        .map(|session_id| session_id.0.as_raw())
        .unwrap_or_default();
    let vt = world
        .get::<Vt>(entity)
        .map(|vt| vt.0.get())
        .unwrap_or_default();

    if let Some(mut active_state) = world.get_mut::<ActiveState>(entity) {
        if active_state.0 == state {
            return;
        }

        log::info!(session_id, vt; "session state changed");
        active_state.0 = state;
    }
}

impl hyprwire::Handler<session_management_v1::SessionManagementV1> for crate::Sessiond {
    fn protocol_filter(&mut self, client: server::ServerClient) -> bool {
        self.world
            .resource::<access::AccessRules>()
            .management
            .allows(client.creds())
    }
}

impl hyprwire::Handler<session_management_v2::SessionManagementV2> for crate::Sessiond {
    fn protocol_filter(&mut self, client: server::ServerClient) -> bool {
        self.world
            .resource::<access::AccessRules>()
            .management
            .allows(client.creds())
    }
}

impl hyprwire::Handler<session_core_v1::SessionCoreV1> for crate::Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        sync_vt_session_states(&mut self.world);

        entities(&self.world)
            .into_iter()
            .filter_map(|entity| id(&self.world, entity))
            .for_each(|session_id| {
                object.send_session_added(session_id.as_raw());
            });
        object.send_done();
        self.world
            .resource_mut::<SessionInfoManagers>()
            .0
            .push(object);
    }
}

impl crate::Sessiond {
    pub fn user_config(&self, username: &str) -> config::UserConfig {
        self.world
            .resource::<config::DaemonConfig>()
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
        let client = object.client();

        match event {
            session_manager::Event::Destroy => {}
            session_manager::Event::CreateSession {
                session_management_session,
                uid,
            } => {
                let session_id = self.world.resource_mut::<NextSessionId>().take();
                let username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(); "session created");

                let leader_id = client.creds().pid.as_raw_pid();

                if let Err(e) = self
                    .world
                    .resource_mut::<cgroup::CgroupManager>()
                    .create_session_cgroup(uid, session_id, leader_id)
                {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                vt::refresh_current_vts(&mut self.world);

                user::create_runtime_dir(uid, &self.user_config(&username).runtime_dir(uid));

                spawn(
                    &mut self.world,
                    session_id,
                    SessionClass::User,
                    uid,
                    username,
                    None,
                    leader_id,
                    None,
                    session_core_v1::ActiveState::Opening,
                    Some(session_management_session),
                );
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
        let client = object.client();

        match event {
            session_manager_v1::Event::CreateSession {
                uid,
                vt,
                is_local,
                remote_host,
                class,
            } => {
                let session_id = self.world.resource_mut::<NextSessionId>().take();
                let username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(), vt, is_local, remote_host = remote_host.as_str(); "session created");

                if let Err(e) = self
                    .world
                    .resource_mut::<cgroup::CgroupManager>()
                    .create_session_cgroup(uid, session_id, client.creds().pid.as_raw_pid())
                {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                vt::refresh_current_vts(&mut self.world);

                let is_local = is_local != 0;
                let session_vt = num::NonZeroU32::new(vt);

                let active_state =
                    if session_vt.is_some_and(|vt| vt::current_vt().ok() == Some(vt)) && is_local {
                        session_core_v1::ActiveState::Active
                    } else {
                        session_core_v1::ActiveState::Online
                    };

                user::create_runtime_dir(uid, &self.user_config(&username).runtime_dir(uid));

                let pending_env = self
                    .world
                    .resource_mut::<PendingSessionEnvs>()
                    .0
                    .remove(object);

                let mut env = self.user_config(&username).environment(
                    uid,
                    session_id.as_raw(),
                    pending_env
                        .as_ref()
                        .and_then(|env| env.get("TERM"))
                        .map(String::as_str),
                );
                env.extend(session_envs_by_uid(&self.world, uid, None));

                let session_entity = spawn(
                    &mut self.world,
                    session_id,
                    class.into(),
                    uid,
                    username,
                    session_vt.map(num::NonZeroU32::get),
                    client.creds().pid.as_raw_pid(),
                    if is_local { None } else { Some(remote_host) },
                    active_state,
                    None,
                );

                if let Some(pending_env) = pending_env
                    && let Some(mut session_env) = self.world.get_mut::<SessionEnv>(session_entity)
                {
                    session_env.0 = pending_env;
                }

                object.send_session_id(session_id.as_raw());
                for (name, value) in env {
                    object.send_env(name.as_str(), value.as_str());
                }
                object.send_done();

                for manager in &self.world.resource::<SessionInfoManagers>().0 {
                    manager.send_session_added(session_id.as_raw());
                }
            }
            session_manager_v1::Event::Env { name, value } => {
                self.world
                    .resource_mut::<PendingSessionEnvs>()
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
                    .resource_mut::<PendingSessionEnvs>()
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
                    self.world.entity_mut(entity).insert(Vt(vt));
                }
            }
            session_management_session::Event::RemoteHost { remote_host } => {
                self.world
                    .entity_mut(entity)
                    .insert(RemoteHost(remote_host));
                self.world.entity_mut(entity).remove::<IsLocal>();
            }
            session_management_session::Event::Done => {
                let uid = self.world.get::<Uid>(entity).map(|uid| uid.0);
                let username = self
                    .world
                    .get::<Username>(entity)
                    .map(|username| username.0.clone());
                let session_vt = self.world.get::<Vt>(entity).map(|vt| vt.0);
                let is_local = self.world.get::<IsLocal>(entity).is_some();
                vt::refresh_current_vts(&mut self.world);

                let active_state =
                    if session_vt.is_some_and(|vt| vt::current_vt().ok() == Some(vt)) && is_local {
                        session_core_v1::ActiveState::Active
                    } else {
                        session_core_v1::ActiveState::Online
                    };

                set_active_state(&mut self.world, entity, active_state);

                object.send_session_id(session_id.as_raw());
                if let (Some(uid), Some(username)) = (uid, username) {
                    let term = self
                        .world
                        .get::<SessionEnv>(entity)
                        .and_then(|env| env.0.get("TERM"))
                        .map(String::as_str);
                    let mut env =
                        self.user_config(&username)
                            .environment(uid, session_id.as_raw(), term);
                    env.extend(session_envs_by_uid(&self.world, uid, Some(entity)));

                    for (name, value) in env {
                        object.send_env(name.as_str(), value.as_str());
                    }
                }
                object.send_done();

                for manager in &self.world.resource::<SessionInfoManagers>().0 {
                    manager.send_session_added(session_id.as_raw());
                }
            }
            session_management_session::Event::Env { name, value } => {
                if let Some(mut env) = self.world.get_mut::<SessionEnv>(entity) {
                    env.0.insert(name, value);
                } else {
                    self.world
                        .entity_mut(entity)
                        .insert(SessionEnv(BTreeMap::from([(name, value)])));
                }
            }
            session_management_session::Event::Class { class } => {
                self.world
                    .entity_mut(entity)
                    .insert(SessionClass::from(class));
            }
            session_management_session::Event::DelegateCgroup {
                session_management_cgroup,
            } => {}
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

                let Ok(Some(session_id)) = self
                    .world
                    .resource::<cgroup::CgroupManager>()
                    .find_session_by_pid(pid)
                else {
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
                let entity = entity_by_uid(&self.world, uid)
                    .or_else(|| user::create(&mut self.world, uid, false));
                let Some(entity) = entity else {
                    log::warn!(uid; "get_user requested for unknown user");
                    session_core_user.send_not_found();
                    return;
                };

                let management_access = self.world.resource::<access::AccessRules>().management;
                user::add_object(
                    &mut self.world,
                    entity,
                    session_core_user,
                    management_access,
                );
            }
            session_core_manager::Event::Destroy => {
                self.world
                    .resource_mut::<SessionInfoManagers>()
                    .0
                    .retain(|m| m != object);
            }
        }
    }
}
