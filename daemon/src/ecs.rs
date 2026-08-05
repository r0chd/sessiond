use crate::session_core_v1::{session_core_manager, session_core_session, session_core_user};
use crate::session_management_v1::session_manager;
use crate::session_management_v2::session_management_session;
use crate::{
    config, session, session_core_v1, session_management_v1, session_management_v2,
    session_power_v1,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use std::collections::{BTreeMap, HashMap};
use std::{num, time};

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Notify;

#[derive(Resource)]
pub struct AccessRules {
    pub management: crate::Access,
}

#[derive(Resource)]
pub struct DaemonConfig(pub config::Config);

#[derive(Resource, Default)]
pub struct SessionInfoManagers(pub Vec<session_core_manager::SessionCoreManager>);

#[derive(Resource, Default)]
pub struct PowerManagers(pub Vec<session_power_v1::session_power_manager::SessionPowerManager>);

#[derive(Resource, Default)]
pub struct NotifyBatch {
    pub sessions: Vec<Entity>,
    pub users: Vec<Entity>,
}

impl NotifyBatch {
    pub fn session(&mut self, entity: Entity) {
        if !self.sessions.contains(&entity) {
            self.sessions.push(entity);
        }
    }
    pub fn user(&mut self, entity: Entity) {
        if !self.users.contains(&entity) {
            self.users.push(entity);
        }
    }
}

#[derive(Resource)]
pub struct NextSessionId(pub session::SessionId);

impl NextSessionId {
    pub const fn take(&mut self) -> session::SessionId {
        let id = self.0;
        self.0 = self.0.next();
        id
    }
}

#[derive(Resource, Default)]
pub struct PendingSessionEnvs(
    pub HashMap<session_manager::SessionManager, BTreeMap<String, String>>,
);

#[derive(Resource, Default)]
pub struct ProtocolObjects {
    pub sessions: HashMap<Entity, Vec<session_core_session::SessionCoreSession>>,
    pub users: HashMap<Entity, Vec<session_core_user::SessionCoreUser>>,
}

impl ProtocolObjects {
    pub fn session_objects(
        &self,
        entity: Entity,
    ) -> Option<&[session_core_session::SessionCoreSession]> {
        self.sessions.get(&entity).map(Vec::as_slice)
    }
    pub fn add_session_object(
        &mut self,
        entity: Entity,
        object: session_core_session::SessionCoreSession,
    ) {
        self.sessions.entry(entity).or_default().push(object);
    }
    pub fn remove_session_object(
        &mut self,
        entity: Entity,
        object: &session_core_session::SessionCoreSession,
    ) {
        if let Some(objects) = self.sessions.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }
    pub fn remove_session_objects(&mut self, entity: Entity) {
        self.sessions.remove(&entity);
    }
    pub fn session_entity_by_object(
        &self,
        object: &session_core_session::SessionCoreSession,
    ) -> Option<Entity> {
        self.sessions.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }
    pub fn user_objects(&self, entity: Entity) -> Option<&[session_core_user::SessionCoreUser]> {
        self.users.get(&entity).map(Vec::as_slice)
    }
    pub fn add_user_object(&mut self, entity: Entity, object: session_core_user::SessionCoreUser) {
        self.users.entry(entity).or_default().push(object);
    }
    pub fn remove_user_object(
        &mut self,
        entity: Entity,
        object: &session_core_user::SessionCoreUser,
    ) {
        if let Some(objects) = self.users.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }
    pub fn user_entity_by_object(
        &self,
        object: &session_core_user::SessionCoreUser,
    ) -> Option<Entity> {
        self.users.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }
}

pub fn init_resources(world: &mut World) {
    world.insert_resource(ProtocolObjects::default());
    world.insert_resource(NotifyBatch::default());
}

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
    pub active_state: ActiveState,
}

#[derive(Bundle)]
pub struct UserBundle {
    pub marker: User,
    pub uid: Uid,
    pub username: Username,
    pub state: UserState,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct User;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lingering;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsLocal;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uid(pub u32);
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Username(pub String);
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub session::SessionId);
#[derive(Component)]
pub struct UserState(pub session_core_v1::UserState);
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
