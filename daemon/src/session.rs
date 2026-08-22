#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::session_core_v1::{session_core_manager, session_core_session};
use crate::session_management_v1::session_manager as session_manager_v1;
use crate::session_management_v2::{session_management_session, session_manager};
use crate::{
    cgroup, config, session_core_v1, session_management_v1, session_management_v2, user, vt,
};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use hyprwire::server;
use std::collections::{BTreeMap, HashMap};
use std::{num, sync, time};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SessionManagementTarget {
    V1(session_manager_v1::SessionManager),
    V2(session_management_session::SessionManagementSession),
}

#[derive(Clone, Copy)]
enum SessionManagementError {
    NotFound,
    Io,
}

impl SessionManagementTarget {
    fn owned_session(&self) -> Option<session_management_session::SessionManagementSession> {
        match self {
            Self::V1(_) => None,
            Self::V2(object) => Some(object.clone()),
        }
    }

    fn send_session_id(&self, session_id: u32) {
        match self {
            Self::V1(object) => object.send_session_id(session_id),
            Self::V2(object) => object.send_session_id(session_id),
        }
    }

    fn send_env(&self, name: &str, value: &str) {
        match self {
            Self::V1(object) => object.send_env(name, value),
            Self::V2(object) => object.send_env(name, value),
        }
    }

    fn send_done(&self) {
        match self {
            Self::V1(object) => object.send_done(),
            Self::V2(object) => object.send_done(),
        }
    }

    fn send_error(&self, code: SessionManagementError, message: impl AsRef<str>) {
        let message = message.as_ref();
        match self {
            Self::V1(object) => object.send_error(
                match code {
                    SessionManagementError::NotFound => session_management_v1::ErrorCode::NotFound,
                    SessionManagementError::Io => session_management_v1::ErrorCode::Io,
                },
                message,
            ),
            Self::V2(object) => object.send_error(
                match code {
                    SessionManagementError::NotFound => session_management_v2::ErrorCode::NotFound,
                    SessionManagementError::Io => session_management_v2::ErrorCode::Io,
                },
                message,
            ),
        }
    }
}

pub struct PendingSession {
    pub manager: Option<session_manager::SessionManager>,
    pub uid: u32,
    pub class: Option<SessionClass>,
    pub vt: Option<num::NonZeroU32>,
    pub remote_host: Option<sync::Arc<str>>,
    pub env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    pub leader_id: i32,
    pub delegate_cgroup: bool,
}

#[derive(Resource, Default)]
pub struct PendingSessions {
    pub by_management_object: HashMap<SessionManagementTarget, PendingSession>,
}

#[allow(clippy::struct_field_names)]
#[derive(Resource, Default)]
pub struct SessionRegistry {
    pub by_id: HashMap<SessionId, Entity>,
    pub by_object: HashMap<session_core_session::SessionCoreSession, Entity>,
    pub by_uid: HashMap<u32, Vec<Entity>>,
    pub by_management_object: HashMap<session_management_session::SessionManagementSession, Entity>,
}

impl SessionRegistry {
    pub fn entities_by_uid(&self, uid: u32) -> Vec<Entity> {
        self.by_uid.get(&uid).cloned().unwrap_or_default()
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        self.by_id
            .retain(|_, mapped_entity| *mapped_entity != entity);
        self.by_object
            .retain(|_, mapped_entity| *mapped_entity != entity);
        self.by_management_object
            .retain(|_, mapped_entity| *mapped_entity != entity);
        self.by_uid.retain(|_, entities| {
            entities.retain(|mapped_entity| *mapped_entity != entity);
            !entities.is_empty()
        });
    }
}

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
    pub HashMap<session_manager_v1::SessionManager, BTreeMap<sync::Arc<str>, sync::Arc<str>>>,
);

#[derive(Bundle)]
pub struct SessionBundle {
    pub marker: Session,
    pub id: SessionId,
    pub class: SessionClass,
    pub uid: Uid,
    pub username: Username,
    pub timestamp: Timestamp,
    pub leader_id: LeaderId,
    pub env: SessionEnv,
    pub active_state: session_core_v1::ActiveState,
    pub objects: Objects,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsLocal;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uid(pub u32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Username(pub sync::Arc<str>);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vt(pub num::NonZeroU32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp(pub time::SystemTime);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderId(pub i32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RemoteHost(pub sync::Arc<str>);

#[derive(Component)]
pub struct SessionEnv(pub BTreeMap<sync::Arc<str>, sync::Arc<str>>);

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

#[derive(Component, Default)]
pub struct Objects(pub Vec<session_core_session::SessionCoreSession>);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    commands: &mut Commands,
    session_id: SessionId,
    class: SessionClass,
    uid: u32,
    username: sync::Arc<str>,
    vt: Option<u32>,
    leader_id: i32,
    remote_host: Option<sync::Arc<str>>,
    active_state: session_core_v1::ActiveState,
    session_object: Option<session_management_session::SessionManagementSession>,
    env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    cgroup: cgroup::CgroupBundle,
    session_registry: &mut ResMut<SessionRegistry>,
) -> entity::Entity {
    let mut cmd = commands.spawn(SessionBundle {
        marker: Session,
        id: session_id,
        class,
        uid: Uid(uid),
        username: Username(username),
        timestamp: Timestamp(time::SystemTime::now()),
        leader_id: LeaderId(leader_id),
        env: SessionEnv(env),
        active_state,
        objects: Objects::default(),
    });
    cmd.insert(cgroup);
    if let Some(vt) = vt.and_then(num::NonZeroU32::new) {
        cmd.insert(Vt(vt));
    }
    if let Some(remote_host) = remote_host {
        cmd.insert(RemoteHost(sync::Arc::clone(&remote_host)));
    } else {
        cmd.insert(IsLocal);
    }

    if let Some(session_object) = session_object {
        session_registry
            .by_management_object
            .insert(session_object, cmd.id());
    }

    let entity = cmd.id();
    session_registry.by_id.insert(session_id, entity);
    session_registry.by_uid.entry(uid).or_default().push(entity);
    entity
}

pub fn entities(world: &world::World) -> Vec<entity::Entity> {
    let Some(mut sessions) = world.try_query_filtered::<Entity, With<Session>>() else {
        return Vec::new();
    };
    sessions.iter(world).collect()
}

pub fn entity(world: &world::World, session_id: SessionId) -> Option<entity::Entity> {
    world
        .resource::<SessionRegistry>()
        .by_id
        .get(&session_id)
        .copied()
}

pub fn can_modify(world: &world::World, entity: entity::Entity, creds: &hyprwire::Creds) -> bool {
    let session_uid = world.get::<Uid>(entity).map_or(0, |uid| uid.0);
    creds.uid.is_root() || creds.uid.as_raw() == session_uid
}

#[derive(Message)]
pub struct SessionControlMessage {
    pub entity: entity::Entity,
    pub object: Option<session_core_session::SessionCoreSession>,
    pub action: SessionControlAction,
}

pub enum SessionControlAction {
    Terminate { creds: hyprwire::Creds },
    Kill { creds: hyprwire::Creds },
    Lock { creds: hyprwire::Creds },
    Unlock { creds: hyprwire::Creds },
    Destroy,
}

pub fn handle_session_control(
    mut actions: MessageReader<SessionControlMessage>,
    mut session_registry: ResMut<SessionRegistry>,
    mut sessions: Query<
        (
            &SessionId,
            &Uid,
            &mut session_core_v1::ActiveState,
            &Vt,
            &mut Objects,
        ),
        With<Session>,
    >,
    cgroups: Query<(&SessionId, &cgroup::CgroupPath), With<cgroup::Cgroup>>,
    loop_handle: NonSend<'_, calloop::LoopHandle<'static, crate::Sessiond>>,
) {
    for action in actions.read() {
        match action.action {
            SessionControlAction::Terminate { creds } => {
                let Ok((session_id, uid, mut active_state, vt, _)) =
                    sessions.get_mut(action.entity)
                else {
                    continue;
                };
                if !creds.uid.is_root() && creds.uid.as_raw() != uid.0 {
                    if let Some(object) = &action.object {
                        object.send_error(
                            session_core_v1::ErrorCode::PermissionDenied,
                            "permission denied",
                        );
                    }
                    continue;
                }
                log::info!(session_id = session_id.as_raw(); "terminate requested");
                *active_state = session_core_v1::ActiveState::Closing;
                log::info!(session_id = session_id.0, vt = vt.0; "session state changed");
                let Ok((cgroup_session_id, cgroup_path)) = cgroups.get(action.entity) else {
                    continue;
                };
                if let Err(err) =
                    cgroup::terminate(cgroup_path, *cgroup_session_id, *uid, &loop_handle)
                {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
                }
            }
            SessionControlAction::Kill { creds } => {
                let Ok((session_id, uid, mut active_state, vt, _)) =
                    sessions.get_mut(action.entity)
                else {
                    continue;
                };
                if !creds.uid.is_root() && creds.uid.as_raw() != uid.0 {
                    if let Some(object) = &action.object {
                        object.send_error(
                            session_core_v1::ErrorCode::PermissionDenied,
                            "permission denied",
                        );
                    }
                    continue;
                }
                log::info!(session_id = session_id.as_raw(); "kill requested");
                *active_state = session_core_v1::ActiveState::Closing;
                log::info!(session_id = session_id.0, vt = vt.0; "session state changed");
                let Ok((cgroup_session_id, cgroup_path)) = cgroups.get(action.entity) else {
                    continue;
                };
                if let Err(err) = cgroup::kill(cgroup_path, *cgroup_session_id, *uid, &loop_handle)
                {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "kill failed");
                }
            }
            SessionControlAction::Lock { creds } => {
                let Ok((session_id, uid, _, _, objects)) = sessions.get_mut(action.entity) else {
                    continue;
                };
                if !creds.uid.is_root() && creds.uid.as_raw() != uid.0 {
                    if let Some(object) = &action.object {
                        object.send_error(
                            session_core_v1::ErrorCode::PermissionDenied,
                            "permission denied",
                        );
                    }
                    continue;
                }
                for object in &objects.0 {
                    object.send_locked();
                    object.send_ok();
                }
                log::info!(session_id = session_id.as_raw(); "lock requested");
            }
            SessionControlAction::Unlock { creds } => {
                let Ok((session_id, uid, _, _, objects)) = sessions.get_mut(action.entity) else {
                    continue;
                };
                if !creds.uid.is_root() && creds.uid.as_raw() != uid.0 {
                    if let Some(object) = &action.object {
                        object.send_error(
                            session_core_v1::ErrorCode::PermissionDenied,
                            "permission denied",
                        );
                    }
                    continue;
                }
                for object in &objects.0 {
                    object.send_unlocked();
                    object.send_ok();
                }
                log::info!(session_id = session_id.as_raw(); "unlock requested");
            }
            SessionControlAction::Destroy => {
                if let Some(object) = &action.object {
                    session_registry.by_object.remove(object);
                    if let Ok((_, _, _, _, mut objects)) = sessions.get_mut(action.entity) {
                        objects.0.retain(|candidate| candidate != object);
                    }
                }
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = self
            .world
            .resource::<SessionRegistry>()
            .by_object
            .get(object)
            .copied()
        else {
            return;
        };
        let client = object.client();

        match event {
            session_core_session::Event::Terminate => {
                self.world.write_message(SessionControlMessage {
                    entity,
                    object: Some(object.clone()),
                    action: SessionControlAction::Terminate {
                        creds: *client.creds(),
                    },
                });
            }
            session_core_session::Event::Kill => {
                self.world.write_message(SessionControlMessage {
                    entity,
                    object: Some(object.clone()),
                    action: SessionControlAction::Kill {
                        creds: *client.creds(),
                    },
                });
            }
            session_core_session::Event::Lock => {
                self.world.write_message(SessionControlMessage {
                    entity,
                    object: Some(object.clone()),
                    action: SessionControlAction::Lock {
                        creds: *client.creds(),
                    },
                });
            }
            session_core_session::Event::Unlock => {
                self.world.write_message(SessionControlMessage {
                    entity,
                    object: Some(object.clone()),
                    action: SessionControlAction::Unlock {
                        creds: *client.creds(),
                    },
                });
            }
            session_core_session::Event::Destroy => {
                self.world.write_message(SessionControlMessage {
                    entity,
                    object: Some(object.clone()),
                    action: SessionControlAction::Destroy,
                });
            }
        }
    }
}

impl crate::Sessiond {
    pub fn terminate_session(&mut self, entity: entity::Entity) {
        let Some(session_id) = self.world.get::<SessionId>(entity).copied() else {
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
        let Some(path) = self.world.get::<cgroup::CgroupPath>(entity) else {
            log::error!(session_id = session_id.as_raw(); "session has no cgroup");
            return;
        };
        let Some(cgroup_session_id) = self.world.get::<SessionId>(entity) else {
            log::error!(session_id = session_id.as_raw(); "session has no cgroup id");
            return;
        };
        let Some(uid) = self.world.get::<Uid>(entity) else {
            log::error!(session_id = session_id.as_raw(); "session has no uid");
            return;
        };
        if let Err(err) = cgroup::terminate(path, *cgroup_session_id, *uid, loop_handle) {
            log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
        }
    }

    pub(crate) fn finish_session_destroy(&mut self, session_id: SessionId) {
        let Some(entity) = entity(&self.world, session_id) else {
            log::warn!(session_id = session_id.as_raw(); "destroy requested for unknown session");
            return;
        };

        let uid = self.world.get::<Uid>(entity).map_or(0, |uid| uid.0);
        let username = self
            .world
            .get::<Username>(entity)
            .map_or("", |username| username.0.as_ref());
        log::info!(session_id = session_id.as_raw(), uid, username; "session destroyed");

        if let Some(objects) = self.world.get::<Objects>(entity) {
            for object in &objects.0 {
                object.send_destroyed();
            }
        }

        self.world
            .resource_mut::<SessionRegistry>()
            .remove_entity(entity);
        self.world.despawn(entity);
        #[cfg(feature = "consolekit")]
        self.world
            .resource::<consolekit::ConsoleKit>()
            .0
            .remove_session(session_id);

        self.cleanup_user_runtime_dir(uid);
    }

    pub(crate) fn cleanup_user_runtime_dir(&self, uid: u32) {
        let Some(user) = uzers::get_user_by_uid(uid) else {
            return;
        };

        if !user::is_lingering(&self.world, uid)
            && self
                .world
                .resource::<SessionRegistry>()
                .entities_by_uid(uid)
                .is_empty()
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
    if let Some(active_state) = world.get::<session_core_v1::ActiveState>(entity) {
        object.send_active(*active_state);
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

    if let Some(mut objects) = world.get_mut::<Objects>(entity) {
        objects.0.push(object.clone());
    }
    world
        .resource_mut::<SessionRegistry>()
        .by_object
        .insert(object, entity);
}

pub fn set_active_state(
    world: &mut world::World,
    entity: entity::Entity,
    state: session_core_v1::ActiveState,
) {
    let session_id = world
        .get::<SessionId>(entity)
        .map(|session_id| session_id.as_raw())
        .unwrap_or_default();
    let vt = world
        .get::<Vt>(entity)
        .map(|vt| vt.0.get())
        .unwrap_or_default();

    if let Some(mut active_state) = world.get_mut::<session_core_v1::ActiveState>(entity) {
        if *active_state == state {
            return;
        }

        log::info!(session_id, vt; "session state changed");
        *active_state = state;
    }
}

impl hyprwire::Handler<session_management_v1::SessionManagementV1> for crate::Sessiond {
    fn protocol_filter(&mut self, client: server::ServerClient) -> bool {
        client.creds().uid.is_root()
    }
}

impl hyprwire::Handler<session_management_v2::SessionManagementV2> for crate::Sessiond {
    fn protocol_filter(&mut self, client: server::ServerClient) -> bool {
        client.creds().uid.is_root()
    }
}

impl hyprwire::Handler<session_core_v1::SessionCoreV1> for crate::Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        entities(&self.world)
            .into_iter()
            .filter_map(|entity| self.world.get::<SessionId>(entity))
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
            session_manager::Event::Destroy => {
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::ManagerDestroy {
                        management_object: object.clone(),
                    },
                    object: None,
                });
            }
            session_manager::Event::CreateSession {
                session_management_session,
                uid,
            } => {
                let leader_id = client.creds().pid.as_raw_pid();
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Create {
                        management_object: Some(object.clone()),
                        uid,
                        leader_id,
                    },
                    object: Some(SessionManagementTarget::V2(session_management_session)),
                });
            }
        }
    }
}

#[derive(Message)]
pub struct SessionManagementMessage {
    action: SessionManagementAction,
    object: Option<SessionManagementTarget>,
}

pub enum SessionManagementAction {
    Create {
        management_object: Option<session_manager::SessionManager>,
        uid: u32,
        leader_id: i32,
    },
    ManagerDestroy {
        management_object: session_manager::SessionManager,
    },
    Env {
        key: sync::Arc<str>,
        value: sync::Arc<str>,
    },
    Vt {
        vt: u32,
    },
    DelegateCgroup,
    RemoteHost {
        remote_host: sync::Arc<str>,
    },
    Class {
        class: SessionClass,
    },
    Done,
    Destroy,
}

pub fn handle_session_management_action(
    mut commands: Commands,
    mut actions: MessageReader<SessionManagementMessage>,
    mut pending_sessions: ResMut<PendingSessions>,
    mut next_id: ResMut<NextSessionId>,
    cgroup_manager: Res<cgroup::CgroupManager>,
    info_managers: Res<SessionInfoManagers>,
    mut session_registry: ResMut<SessionRegistry>,
    sessions: Query<(&SessionEnv, &Uid), With<Session>>,
    config: Res<config::DaemonConfig>,
) {
    for action in actions.read() {
        if let SessionManagementAction::ManagerDestroy { management_object } = &action.action {
            pending_sessions
                .by_management_object
                .retain(|_, pending| pending.manager.as_ref() != Some(management_object));
            continue;
        }

        let Some(object) = action.object.as_ref() else {
            continue;
        };

        match &action.action {
            SessionManagementAction::ManagerDestroy { .. } => unreachable!(),
            SessionManagementAction::Create {
                management_object,
                uid,
                leader_id,
            } => {
                pending_sessions.by_management_object.insert(
                    object.clone(),
                    PendingSession {
                        manager: management_object.clone(),
                        uid: *uid,
                        class: None,
                        vt: None,
                        remote_host: None,
                        env: BTreeMap::new(),
                        leader_id: *leader_id,
                        delegate_cgroup: false,
                    },
                );
                continue;
            }
            SessionManagementAction::Env { key, value } => {
                if let Some(pending_session) = pending_sessions.by_management_object.get_mut(object)
                {
                    pending_session
                        .env
                        .insert(sync::Arc::clone(key), sync::Arc::clone(value));
                    continue;
                }
            }
            SessionManagementAction::Vt { vt } => {
                if let Some(pending_session) = pending_sessions.by_management_object.get_mut(object)
                {
                    pending_session.vt = num::NonZeroU32::new(*vt);
                    continue;
                }
            }
            SessionManagementAction::RemoteHost { remote_host } => {
                if let Some(pending_session) = pending_sessions.by_management_object.get_mut(object)
                {
                    pending_session.remote_host = Some(sync::Arc::clone(remote_host));
                    continue;
                }
            }
            SessionManagementAction::Class { class } => {
                if let Some(pending_session) = pending_sessions.by_management_object.get_mut(object)
                {
                    pending_session.class = Some(*class);
                    continue;
                }
            }
            SessionManagementAction::DelegateCgroup => {
                if let Some(pending_session) = pending_sessions.by_management_object.get_mut(object)
                {
                    pending_session.delegate_cgroup = true;
                    continue;
                }
            }
            SessionManagementAction::Done => {
                if let Some(pending_session) = pending_sessions.by_management_object.remove(object)
                {
                    let Some(user) = uzers::get_user_by_uid(pending_session.uid) else {
                        object.send_error(
                            SessionManagementError::NotFound,
                            format!("user {} does not exist", pending_session.uid),
                        );
                        return;
                    };
                    let username = user.name().to_string_lossy().into();

                    let session_id = next_id.take();

                    let cgroup = match cgroup_manager.create_session_cgroup(
                        &user,
                        session_id,
                        pending_session.leader_id,
                    ) {
                        Ok(cgroup) => cgroup,
                        Err(e) => {
                            log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                            object.send_error(
                                SessionManagementError::Io,
                                format!("failed to create session cgroup: {e}"),
                            );
                            return;
                        }
                    };

                    if pending_session.delegate_cgroup
                        && let Err(error) = cgroup::CgroupManager::delegate_cgroup(&user, &cgroup)
                    {
                        if let Err(cleanup_error) =
                            cgroup::CgroupManager::remove_session_cgroup(&cgroup.path.0)
                        {
                            log::error!(
                                session_id = session_id.as_raw(),
                                error = cleanup_error.to_string().as_str();
                                "failed to clean up session cgroup after delegation failure"
                            );
                        }
                        object.send_error(
                            SessionManagementError::Io,
                            format!("failed to delegate session cgroup: {error}"),
                        );
                        return;
                    }

                    let user_config = config.0.users.get(&username).cloned().unwrap_or_default();
                    user::create_runtime_dir(
                        pending_session.uid,
                        &user_config.runtime_dir(pending_session.uid),
                    );

                    let is_local = pending_session.remote_host.is_none();
                    let active_state = if pending_session
                        .vt
                        .is_some_and(|vt| vt::current_vt().ok() == Some(vt))
                        && is_local
                    {
                        session_core_v1::ActiveState::Active
                    } else {
                        session_core_v1::ActiveState::Online
                    };
                    let term = pending_session.env.get("TERM").cloned();
                    let mut env =
                        user_config.environment(pending_session.uid, session_id.as_raw(), term);
                    for (session_env, _) in sessions
                        .iter()
                        .filter(|(_, uid)| uid.0 == pending_session.uid)
                    {
                        env.extend(session_env.0.clone());
                    }
                    env.extend(pending_session.env.clone());
                    let protocol_env = env.clone();

                    spawn(
                        &mut commands,
                        session_id,
                        pending_session.class.unwrap_or_default(),
                        pending_session.uid,
                        username,
                        pending_session.vt.map(num::NonZeroU32::get),
                        pending_session.leader_id,
                        pending_session.remote_host,
                        active_state,
                        object.owned_session(),
                        env,
                        cgroup,
                        &mut session_registry,
                    );

                    for (name, value) in protocol_env {
                        object.send_env(name.as_ref(), value.as_ref());
                    }

                    object.send_session_id(session_id.as_raw());
                    object.send_done();

                    for manager in &info_managers.0 {
                        manager.send_session_added(session_id.as_raw());
                    }

                    continue;
                }
            }
            SessionManagementAction::Destroy => {
                pending_sessions.by_management_object.remove(object);
                continue;
            }
        }

        object.send_error(SessionManagementError::Io, "session is no longer pending");
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
                let pending_env = self
                    .world
                    .resource_mut::<PendingSessionEnvs>()
                    .0
                    .remove(object)
                    .unwrap_or_default();
                let target = SessionManagementTarget::V1(object.clone());

                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Create {
                        management_object: None,
                        uid,
                        leader_id: client.creds().pid.as_raw_pid(),
                    },
                    object: Some(target.clone()),
                });
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Vt { vt },
                    object: Some(target.clone()),
                });
                if is_local == 0 {
                    self.world.write_message(SessionManagementMessage {
                        action: SessionManagementAction::RemoteHost {
                            remote_host: remote_host.into(),
                        },
                        object: Some(target.clone()),
                    });
                }
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Class {
                        class: SessionClass::from(class),
                    },
                    object: Some(target.clone()),
                });
                for (key, value) in pending_env {
                    self.world.write_message(SessionManagementMessage {
                        action: SessionManagementAction::Env { key, value },
                        object: Some(target.clone()),
                    });
                }
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Done,
                    object: Some(target),
                });
            }
            session_manager_v1::Event::Env { name, value } => {
                self.world
                    .resource_mut::<PendingSessionEnvs>()
                    .0
                    .entry(object.clone())
                    .or_default()
                    .insert(name.into(), value.into());
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
        let Some(entity) = self
            .world
            .resource::<SessionRegistry>()
            .by_management_object
            .get(object)
            .copied()
        else {
            let action = match event {
                session_management_session::Event::Vt { vt } => SessionManagementAction::Vt { vt },
                session_management_session::Event::RemoteHost { remote_host } => {
                    SessionManagementAction::RemoteHost {
                        remote_host: remote_host.into(),
                    }
                }
                session_management_session::Event::Class { class } => {
                    SessionManagementAction::Class {
                        class: SessionClass::from(class),
                    }
                }
                session_management_session::Event::Env { name, value } => {
                    SessionManagementAction::Env {
                        key: name.into(),
                        value: value.into(),
                    }
                }
                session_management_session::Event::DelegateCgroup => {
                    SessionManagementAction::DelegateCgroup
                }
                session_management_session::Event::Done => SessionManagementAction::Done,
                session_management_session::Event::Destroy => SessionManagementAction::Destroy,
            };
            self.world.write_message(SessionManagementMessage {
                action,
                object: Some(SessionManagementTarget::V2(object.clone())),
            });
            return;
        };

        match event {
            session_management_session::Event::Destroy => {
                self.terminate_session(entity);
            }
            _ => {
                object.error(
                    session_management_v2::ErrorCode::Io as u32,
                    "session is no longer pending",
                );
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
                if let Some(entity) = self
                    .world
                    .resource::<SessionRegistry>()
                    .entities_by_uid(uid)
                    .into_iter()
                    .find(|entity| {
                        self.world
                            .get::<SessionClass>(*entity)
                            .is_some_and(|class| *class == SessionClass::User)
                    })
                    && let Some(session_id) = self.world.get::<SessionId>(entity).copied()
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
                let Some(entity) = self
                    .world
                    .resource::<user::UserRegistry>()
                    .by_uid
                    .get(&uid)
                    .copied()
                else {
                    log::warn!(uid; "get_user requested for unknown user");
                    session_core_user.send_not_found();
                    return;
                };

                user::add_object(&mut self.world, entity, session_core_user);
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
