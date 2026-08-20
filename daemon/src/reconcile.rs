#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::{session, session_core_v1, user};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use std::mem;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Notify;

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

#[cfg(feature = "consolekit")]
pub fn sync_consolekit_sessions(
    consolekit: Res<consolekit::ConsoleKit>,
    sessions: Query<
        (
            &session::SessionId,
            &session::Uid,
            &session::Timestamp,
            Option<&session::IsLocal>,
            &session_core_v1::ActiveState,
        ),
        (
            With<session::Session>,
            Or<(
                Added<session::Session>,
                Changed<session_core_v1::ActiveState>,
            )>,
        ),
    >,
) {
    for (session_id, uid, timestamp, is_local, active_state) in &sessions {
        consolekit.0.upsert_session(consolekit::SessionInfo {
            id: *session_id,
            uid: uid.0,
            timestamp: timestamp.0,
            is_local: is_local.is_some(),
            active: *active_state,
        });
    }
}

pub fn notify_session_active_states(
    mut batch: ResMut<NotifyBatch>,
    sessions: Query<
        (Entity, &session_core_v1::ActiveState, &session::Objects),
        (
            With<session::Session>,
            Changed<session_core_v1::ActiveState>,
        ),
    >,
    users: Query<
        (Entity, &session_core_v1::UserState, &user::Objects),
        (With<user::User>, Changed<session_core_v1::UserState>),
    >,
) {
    for (entity, state, objects) in &sessions {
        for object in &objects.0 {
            object.send_active(*state);
        }
        batch.session(entity);
    }

    for (entity, state, objects) in &users {
        for object in &objects.0 {
            object.send_state(*state);
        }
        batch.user(entity);
    }
}

pub fn send_done(
    mut batch: ResMut<NotifyBatch>,
    sessions: Query<&session::Objects>,
    users: Query<&user::Objects>,
) {
    for entity in mem::take(&mut batch.sessions) {
        if let Ok(objects) = sessions.get(entity) {
            for object in &objects.0 {
                object.send_done();
            }
        }
    }
    for entity in mem::take(&mut batch.users) {
        if let Ok(objects) = users.get(entity) {
            for object in &objects.0 {
                object.send_done();
            }
        }
    }
}
