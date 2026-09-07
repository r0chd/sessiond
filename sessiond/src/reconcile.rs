#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::{session, user};
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
            &session::ActiveState,
        ),
        (
            With<session::Session>,
            Or<(Added<session::Session>, Changed<session::ActiveState>)>,
        ),
    >,
) {
    for (session_id, uid, timestamp, is_local, active_state) in &sessions {
        consolekit.0.upsert_session(consolekit::SessionInfo {
            id: *session_id,
            uid: uid.0,
            timestamp: timestamp.0,
            is_local: is_local.is_some(),
            active: active_state.0,
        });
    }
}

pub fn notify_session_active_states(
    mut batch: ResMut<NotifyBatch>,
    sessions: Query<
        (Entity, &session::ActiveState, &session::Objects),
        (With<session::Session>, Changed<session::ActiveState>),
    >,
    users: Query<
        (Entity, &user::UserState, &user::Objects),
        (With<user::User>, Changed<user::UserState>),
    >,
) {
    for (entity, state, objects) in &sessions {
        for object in &objects.0 {
            object.send_active(state.0);
        }
        batch.session(entity);
    }

    for (entity, state, objects) in &users {
        for object in &objects.0 {
            object.send_state(state.0);
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
