#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::{protocol, session, user};
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
            &session::SessionIdComponent,
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
            id: session_id.0,
            uid: uid.0,
            timestamp: timestamp.0,
            is_local: is_local.is_some(),
            active: active_state.0,
        });
    }
}

pub fn notify_session_active_states(
    objects: Res<protocol::ProtocolObjects>,
    mut batch: ResMut<NotifyBatch>,
    sessions: Query<
        (Entity, &session::ActiveState),
        (With<session::Session>, Changed<session::ActiveState>),
    >,
    users: Query<(Entity, &user::UserState), (With<user::User>, Changed<user::UserState>)>,
) {
    for (entity, state) in &sessions {
        if let Some(objects) = objects.session_objects(entity) {
            for object in objects {
                object.send_active(state.0);
            }
            batch.session(entity);
        }
    }

    for (entity, state) in &users {
        if let Some(objects) = objects.user_objects(entity) {
            for object in objects {
                object.send_state(state.0);
            }
            batch.user(entity);
        }
    }
}

pub fn send_done(objects: Res<protocol::ProtocolObjects>, mut batch: ResMut<NotifyBatch>) {
    for entity in mem::take(&mut batch.sessions) {
        if let Some(objects) = objects.session_objects(entity) {
            for object in objects {
                object.send_done();
            }
        }
    }
    for entity in mem::take(&mut batch.users) {
        if let Some(objects) = objects.user_objects(entity) {
            for object in objects {
                object.send_done();
            }
        }
    }
}
