use crate::ecs;
use bevy_ecs::prelude::*;
use std::mem;

#[cfg(feature = "consolekit")]
pub fn sync_consolekit_sessions(
    consolekit: Res<ecs::ConsoleKit>,
    sessions: Query<
        (
            &ecs::SessionId,
            &ecs::Uid,
            &ecs::Timestamp,
            Option<&ecs::IsLocal>,
            &ecs::ActiveState,
        ),
        (
            With<ecs::Session>,
            Or<(Added<ecs::Session>, Changed<ecs::ActiveState>)>,
        ),
    >,
) {
    for (session_id, uid, timestamp, is_local, active_state) in &sessions {
        consolekit.0.upsert_session(crate::consolekit::SessionInfo {
            id: session_id.0,
            uid: uid.0,
            timestamp: timestamp.0,
            is_local: is_local.is_some(),
            active: active_state.0,
        });
    }
}

pub fn notify_session_active_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    sessions: Query<(Entity, &ecs::ActiveState), (With<ecs::Session>, Changed<ecs::ActiveState>)>,
    users: Query<(Entity, &ecs::UserState), (With<ecs::User>, Changed<ecs::UserState>)>,
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

pub fn send_done(objects: Res<ecs::ProtocolObjects>, mut batch: ResMut<ecs::NotifyBatch>) {
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
