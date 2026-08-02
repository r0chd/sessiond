#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]
use crate::ecs;
use bevy_ecs::prelude::*;
use std::mem;

pub fn notify_schedule() -> Schedule {
    let mut schedule = Schedule::new(ecs::Notify);
    schedule.add_systems((notify_session_active_states, notify_user_states).before(send_done));
    schedule.add_systems(send_done);
    schedule
}

fn notify_session_active_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    sessions: Query<(Entity, &ecs::ActiveState), (With<ecs::Session>, Changed<ecs::ActiveState>)>,
) {
    for (entity, state) in &sessions {
        if let Some(objects) = objects.session_objects(entity) {
            for object in objects {
                object.send_active(state.0);
            }
            batch.session(entity);
        }
    }
}

fn notify_user_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    users: Query<(Entity, &ecs::UserState), (With<ecs::User>, Changed<ecs::UserState>)>,
) {
    for (entity, state) in &users {
        if let Some(objects) = objects.user_objects(entity) {
            for object in objects {
                object.send_state(state.0);
            }
            batch.user(entity);
        }
    }
}

fn send_done(objects: Res<ecs::ProtocolObjects>, mut batch: ResMut<ecs::NotifyBatch>) {
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
