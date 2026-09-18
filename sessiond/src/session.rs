use crate::{cgroup, config, inhibit, user, vt};
use bevy_ecs::prelude::*;
use hyprwire::server;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1::session_core_manager;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1::session_core_session;
use hyprwire_protocols_sessiond::server::session_management::v1::session_management_v1;
use hyprwire_protocols_sessiond::server::session_management::v1::session_management_v1::session_manager as session_manager_v1;
use hyprwire_protocols_sessiond::server::session_management::v2::session_management_v2;
use hyprwire_protocols_sessiond::server::session_management::v2::session_management_v2::session_management_session;
use hyprwire_protocols_sessiond::server::session_management::v2::session_management_v2::session_manager;
use std::collections::{BTreeMap, HashMap};
use std::{num, sync, time};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveState(pub session_core_v1::ActiveState);

impl ActiveState {
    pub const ONLINE: Self = Self(session_core_v1::ActiveState::Online);
    pub const ACTIVE: Self = Self(session_core_v1::ActiveState::Active);
    pub const CLOSING: Self = Self(session_core_v1::ActiveState::Closing);
}

impl From<session_core_v1::ActiveState> for ActiveState {
    fn from(state: session_core_v1::ActiveState) -> Self {
        Self(state)
    }
}

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
    pub gid: u32,
    pub class: Option<SessionClass>,
    pub vt: Option<num::NonZeroU32>,
    pub remote_host: Option<sync::Arc<str>>,
    pub env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    pub leader_id: i32,
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

#[derive(Bundle)]
pub struct SessionBundle {
    pub marker: Session,
    pub id: SessionId,
    pub class: SessionClass,
    pub uid: Uid,
    pub gid: Gid,
    pub username: Username,
    pub timestamp: Timestamp,
    pub leader_id: LeaderId,
    pub env: SessionEnv,
    pub active_state: ActiveState,
    pub objects: Objects,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session;

// Mark sessions that should be recovered across restarts,
// this is only marked for session management v1 protocol
// created sessions
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recoverable;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsLocal;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uid(pub u32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gid(pub u32);

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
                    _ => Self::User,
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
    gid: u32,
    username: sync::Arc<str>,
    vt: Option<u32>,
    leader_id: i32,
    remote_host: Option<sync::Arc<str>>,
    active_state: session_core_v1::ActiveState,
    session_object: Option<session_management_session::SessionManagementSession>,
    recoverable: bool,
    env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    cgroup: cgroup::CgroupBundle,
    session_registry: &mut ResMut<SessionRegistry>,
) -> Entity {
    let mut cmd = commands.spawn(SessionBundle {
        marker: Session,
        id: session_id,
        class,
        uid: Uid(uid),
        gid: Gid(gid),
        username: Username(username),
        timestamp: Timestamp(time::SystemTime::now()),
        leader_id: LeaderId(leader_id),
        env: SessionEnv(env),
        active_state: active_state.into(),
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

    if recoverable {
        cmd.insert(Recoverable);
    }

    let entity = cmd.id();
    session_registry.by_id.insert(session_id, entity);
    session_registry.by_uid.entry(uid).or_default().push(entity);
    entity
}

#[derive(Message)]
pub struct SessionControlMessage {
    pub entity: Entity,
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
            &mut ActiveState,
            Option<&Vt>,
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
                *active_state = ActiveState::CLOSING;
                log::info!(session_id = session_id.0, vt = vt.map_or(0, |vt| vt.0.get()); "session state changed");
                let Ok((_, cgroup_path)) = cgroups.get(action.entity) else {
                    continue;
                };
                if let Err(err) = cgroup::terminate(cgroup_path, *session_id, &loop_handle) {
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
                *active_state = ActiveState::CLOSING;
                log::info!(session_id = session_id.0, vt = vt.map_or(0, |vt| vt.0.get()); "session state changed");
                let Ok((_, cgroup_path)) = cgroups.get(action.entity) else {
                    continue;
                };
                if let Err(err) = cgroup::kill(cgroup_path) {
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
            _ => {}
        }
    }
}

#[derive(Message)]
pub struct AddObject {
    session_id: SessionId,
    object: session_core_session::SessionCoreSession,
    creds: hyprwire::Creds,
}

pub fn handle_add_object(
    mut requests: MessageReader<AddObject>,
    mut sessions: Query<
        (
            Entity,
            &SessionId,
            &Uid,
            &Username,
            Option<&Vt>,
            &SessionClass,
            &ActiveState,
            &LeaderId,
            &Timestamp,
            Option<&IsLocal>,
            Option<&RemoteHost>,
            &mut Objects,
        ),
        With<Session>,
    >,
    mut session_registry: ResMut<SessionRegistry>,
) {
    for request in requests.read() {
        let Some((
            entity,
            session_id,
            uid,
            username,
            vt,
            class,
            state,
            leader_id,
            timestamp,
            is_local,
            remote_host,
            mut objects,
        )) = sessions
            .iter_mut()
            .find(|(_, session_id, _, _, _, _, _, _, _, _, _, _)| {
                **session_id == request.session_id
            })
        else {
            log::warn!(session_id = request.session_id.as_raw(); "get_session_by_pid requested for unknown session");
            request.object.send_destroyed();
            continue;
        };

        log::debug!(session_id = request.session_id.as_raw(), uid = request.object.client().creds().uid.as_raw(); "session info requested");
        request.object.send_session_id(session_id.as_raw());
        request.object.send_uid(uid.0);
        request.object.send_username(&username.0);
        request.object.send_class((*class).into());
        request.object.send_vt(vt.map_or(0, |vt| vt.0.get()));
        request.object.send_active(state.0);
        request.object.send_leader(leader_id.0.cast_unsigned());
        let timestamp_secs = timestamp
            .0
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
        request.object.send_timestamp(timestamp);

        if is_local.is_some() {
            request.object.send_is_local(1);
        } else {
            request.object.send_is_local(0);
        }
        if let Some(remote_host) = remote_host {
            request.object.send_remote_host(&remote_host.0);
        }

        request.object.send_can_modify(u32::from(
            request.creds.uid.is_root() || request.creds.uid.as_raw() == uid.0,
        ));

        request.object.send_done();

        objects.0.push(request.object.clone());
        session_registry
            .by_object
            .insert(request.object.clone(), entity);
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

#[derive(Message)]
pub struct SessionInfoManagerMessage {
    pub object: session_core_manager::SessionCoreManager,
    pub action: SessionInfoManagerAction,
}

pub enum SessionInfoManagerAction {
    Initialize,
    Destroy,
}

pub fn handle_session_info_manager(
    mut actions: MessageReader<SessionInfoManagerMessage>,
    users: Res<user::UserRegistry>,
    sessions: Query<&SessionId, With<Session>>,
    inhibitors: Query<&inhibit::InhibitorId, With<inhibit::Inhibitor>>,
    mut info_managers: ResMut<SessionInfoManagers>,
) {
    for action in actions.read() {
        match action.action {
            SessionInfoManagerAction::Initialize => {
                users
                    .by_uid
                    .keys()
                    .for_each(|uid| action.object.send_user_added(*uid));
                sessions
                    .iter()
                    .for_each(|session_id| action.object.send_session_added(session_id.as_raw()));
                inhibitors.iter().for_each(|inhibitor_id| {
                    action.object.send_inhibitor_added(inhibitor_id.as_raw());
                });
                action.object.send_done();
                info_managers.0.push(action.object.clone());
            }
            SessionInfoManagerAction::Destroy => {
                info_managers.0.retain(|manager| manager != &action.object);
            }
        }
    }
}

impl hyprwire::Handler<session_core_v1::SessionCoreV1> for crate::Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        self.world.write_message(SessionInfoManagerMessage {
            object,
            action: SessionInfoManagerAction::Initialize,
        });
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
            _ => {}
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
    mut session_control: MessageWriter<SessionControlMessage>,
    mut pending_sessions: ResMut<PendingSessions>,
    mut next_id: ResMut<NextSessionId>,
    cgroup_manager: Res<cgroup::CgroupManager>,
    info_managers: Res<SessionInfoManagers>,
    mut session_registry: ResMut<SessionRegistry>,
    sessions: Query<(&SessionEnv, &Uid), With<Session>>,
    config: Res<config::Config>,
    loop_handle: NonSend<'_, calloop::LoopHandle<'static, crate::Sessiond>>,
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

        if let SessionManagementTarget::V2(management_object) = object
            && let Some(entity) = session_registry
                .by_management_object
                .get(management_object)
                .copied()
        {
            match action.action {
                SessionManagementAction::Destroy => {
                    session_control.write(SessionControlMessage {
                        entity,
                        object: None,
                        action: SessionControlAction::Terminate {
                            creds: *management_object.client().creds(),
                        },
                    });
                }
                _ => {
                    management_object.error(
                        session_management_v2::ErrorCode::Io as u32,
                        "session is no longer pending",
                    );
                }
            }
            continue;
        }

        match &action.action {
            SessionManagementAction::ManagerDestroy { .. } => unreachable!(),
            SessionManagementAction::Create {
                management_object,
                uid,
                leader_id,
            } => {
                if let Some(user) = uzers::get_user_by_uid(*uid) {
                    pending_sessions.by_management_object.insert(
                        object.clone(),
                        PendingSession {
                            manager: management_object.clone(),
                            uid: *uid,
                            gid: user.primary_group_id(),
                            class: None,
                            vt: None,
                            remote_host: None,
                            env: BTreeMap::new(),
                            leader_id: *leader_id,
                        },
                    );
                }
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
                    if let Err(error) = cgroup::watch_empty(
                        &cgroup.path,
                        session_id,
                        Uid(pending_session.uid),
                        &loop_handle,
                    ) {
                        log::error!(session_id = session_id.as_raw(), error = error.to_string().as_str(); "failed to watch session cgroup");
                        object.send_error(
                            SessionManagementError::Io,
                            "failed to watch session cgroup",
                        );
                        return;
                    }

                    let user_config = config.users.get(&username).cloned().unwrap_or_default();
                    user::create_runtime_dir(
                        pending_session.uid,
                        &user_config.runtime_dir(pending_session.uid, pending_session.gid),
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
                    let mut env = user_config.environment(
                        pending_session.uid,
                        pending_session.gid,
                        session_id.as_raw(),
                        term,
                    );
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
                        pending_session.gid,
                        username,
                        pending_session.vt.map(num::NonZeroU32::get),
                        pending_session.leader_id,
                        pending_session.remote_host,
                        active_state,
                        object.owned_session(),
                        matches!(object, SessionManagementTarget::V1(_)),
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
                env,
            } => {
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
                for env in env {
                    let Some((key, value)) = env.split_once('=') else {
                        continue;
                    };
                    self.world.write_message(SessionManagementMessage {
                        action: SessionManagementAction::Env {
                            key: key.into(),
                            value: value.into(),
                        },
                        object: Some(target.clone()),
                    });
                }
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Done,
                    object: Some(target),
                });
            }
            session_manager_v1::Event::DestroySession { session_id } => {
                if let Some(entity) = self
                    .world
                    .resource::<SessionRegistry>()
                    .by_id
                    .get(&SessionId::from_raw(session_id))
                    .copied()
                {
                    self.world.write_message(SessionControlMessage {
                        entity,
                        object: None,
                        action: SessionControlAction::Terminate {
                            creds: *client.creds(),
                        },
                    });
                }
            }
            session_manager_v1::Event::Destroy => {
                self.world.write_message(SessionManagementMessage {
                    action: SessionManagementAction::Destroy,
                    object: Some(SessionManagementTarget::V1(object.clone())),
                });
            }
            _ => {}
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
        let action = match event {
            session_management_session::Event::Vt { vt } => SessionManagementAction::Vt { vt },
            session_management_session::Event::RemoteHost { remote_host } => {
                SessionManagementAction::RemoteHost {
                    remote_host: remote_host.into(),
                }
            }
            session_management_session::Event::Class { class } => SessionManagementAction::Class {
                class: SessionClass::from(class),
            },
            session_management_session::Event::Env { name, value } => {
                SessionManagementAction::Env {
                    key: name.into(),
                    value: value.into(),
                }
            }
            session_management_session::Event::Done => SessionManagementAction::Done,
            session_management_session::Event::Destroy => SessionManagementAction::Destroy,
            _ => return,
        };
        self.world.write_message(SessionManagementMessage {
            action,
            object: Some(SessionManagementTarget::V2(object.clone())),
        });
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::GetCurrentSession {
                session_core_session,
            } => {
                let pid = object.client().creds().pid.as_raw_pid().cast_unsigned();
                let Ok(Some(session_id)) = self
                    .world
                    .resource::<cgroup::CgroupManager>()
                    .find_session_by_pid(pid)
                else {
                    log::warn!(pid; "get_current_session: no session found for pid");
                    session_core_session.send_destroyed();
                    return;
                };

                self.world.write_message(AddObject {
                    session_id,
                    object: session_core_session,
                    creds: *object.client().creds(),
                });
            }
            session_core_manager::Event::GetSession {
                session_core_session,
                session_id,
            } => {
                self.world.write_message(AddObject {
                    session_id: SessionId::from_raw(session_id),
                    object: session_core_session,
                    creds: *object.client().creds(),
                });
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

                self.world.write_message(AddObject {
                    session_id,
                    object: session_core_session,
                    creds: *object.client().creds(),
                });
            }
            session_core_manager::Event::GetDisplaySessionByUid {
                session_core_session,
                uid,
            } => {
                if let Some((_, session_id)) = self
                    .world
                    .resource::<SessionRegistry>()
                    .entities_by_uid(uid)
                    .into_iter()
                    .filter(|entity| {
                        self.world
                            .get::<SessionClass>(*entity)
                            .is_some_and(|class| *class == SessionClass::User)
                            && self
                                .world
                                .get::<ActiveState>(*entity)
                                .is_some_and(|state| *state == ActiveState::ACTIVE)
                    })
                    .filter_map(|entity| {
                        let timestamp = self.world.get::<Timestamp>(entity)?.0;
                        let session_id = *self.world.get::<SessionId>(entity)?;
                        Some((timestamp, session_id))
                    })
                    .min_by_key(|(timestamp, session_id)| (*timestamp, session_id.as_raw()))
                {
                    self.world.write_message(AddObject {
                        session_id,
                        object: session_core_session,
                        creds: *object.client().creds(),
                    });
                    return;
                }

                log::warn!(uid; "get_display_session_by_uid requested without an active user session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetUser {
                session_core_user,
                uid,
            } => {
                self.world.write_message(user::AddObject {
                    uid,
                    object: session_core_user,
                    creds: *object.client().creds(),
                });
            }
            session_core_manager::Event::GetInhibitor {
                session_core_inhibitor,
                inhibitor_id,
            } => {
                self.world.write_message(inhibit::AddObject {
                    inhibitor_id: inhibit::InhibitorId::from_raw(inhibitor_id),
                    object: session_core_inhibitor,
                });
            }
            session_core_manager::Event::Destroy => {
                self.world.write_message(SessionInfoManagerMessage {
                    object: object.clone(),
                    action: SessionInfoManagerAction::Destroy,
                });
            }
            _ => {}
        }
    }
}
