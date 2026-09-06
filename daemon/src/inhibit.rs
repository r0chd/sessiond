use crate::{cgroup, session};
use bevy_ecs::prelude::*;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::server::session_inhibit::v1::session_inhibit_v1;
use hyprwire_protocols_sessiond::server::session_inhibit::v1::session_inhibit_v1::session_inhibit_manager;
use hyprwire_protocols_sessiond::server::session_inhibit::v1::session_inhibit_v1::session_inhibitor;
use std::fs;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inhibitor;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InhibitorId(u32);

impl InhibitorId {
    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub const fn as_raw(self) -> u32 {
        self.0
    }

    const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[derive(Resource)]
pub struct NextInhibitorId(pub InhibitorId);

impl NextInhibitorId {
    pub const fn take(&mut self) -> InhibitorId {
        let id = self.0;
        self.0 = self.0.next();
        id
    }
}

#[derive(Component, Debug)]
pub struct Shutdown;

#[derive(Component, Debug)]
pub struct Sleep;

#[derive(Component, Debug)]
pub struct Idle;

#[derive(Component, Debug)]
pub struct HandlePowerKey;

#[derive(Component, Debug)]
pub struct HandleSuspendKey;

#[derive(Component, Debug)]
pub struct HandleHibernateKey;

#[derive(Component, Debug)]
pub struct HandleLidKey;

#[derive(Component, Debug)]
pub struct HandleRebootKey;

#[derive(Component, Debug)]
pub struct Who(pub(crate) String);

#[derive(Component, Debug)]
pub struct Why(pub(crate) String);

#[derive(Component, Debug)]
pub struct What(pub(crate) u32);

#[derive(Component, Debug, Clone, Copy)]
pub struct Uid(pub(crate) u32);

#[derive(Component, Debug)]
pub struct Username(pub(crate) String);

#[derive(Component, Debug, Clone, Copy)]
pub struct Pid(pub(crate) u32);

#[derive(Component, Debug)]
pub struct Comm(pub(crate) String);

#[derive(Component, Default)]
pub struct Objects(pub Vec<session_core_v1::session_core_inhibitor::SessionCoreInhibitor>);

#[derive(Bundle)]
pub struct InhibitorBundle {
    pub marker: Inhibitor,
    pub id: InhibitorId,
    pub what: What,
    pub who: Who,
    pub why: Why,
    pub uid: Uid,
    pub username: Username,
    pub pid: Pid,
    pub comm: Comm,
    pub mode: session_inhibit_v1::Mode,
    pub object: session_inhibitor::SessionInhibitor,
    pub objects: Objects,
}

impl hyprwire::Handler<session_inhibit_v1::SessionInhibitV1> for crate::Sessiond {
    fn protocol_filter(&mut self, client: hyprwire::server::ServerClient) -> bool {
        let creds = client.creds();
        if creds.uid.is_root() {
            return true;
        }

        let pid = creds.pid.as_raw_pid().cast_unsigned();
        let Ok(Some(session_id)) = self
            .world
            .resource::<cgroup::CgroupManager>()
            .find_session_by_pid(pid)
        else {
            return false;
        };
        let Some(entity) = self
            .world
            .resource::<session::SessionRegistry>()
            .by_id
            .get(&session_id)
            .copied()
        else {
            return false;
        };

        self.world.get::<session::IsLocal>(entity).is_some()
            && self
                .world
                .get::<session::Uid>(entity)
                .is_some_and(|uid| uid.0 == creds.uid.as_raw())
    }
}

impl hyprwire::Dispatch<session_inhibit_manager::SessionInhibitManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_inhibit_manager::SessionInhibitManager,
        event: <session_inhibit_manager::SessionInhibitManager as hyprwire::Object>::Event<'_>,
    ) {
        const KNOWN_WHAT: u32 = session_inhibit_v1::What::Shutdown as u32
            | session_inhibit_v1::What::Sleep as u32
            | session_inhibit_v1::What::Idle as u32
            | session_inhibit_v1::What::HandlePowerKey as u32
            | session_inhibit_v1::What::HandleSuspendKey as u32
            | session_inhibit_v1::What::HandleHibernateKey as u32
            | session_inhibit_v1::What::HandleLidSwitch as u32
            | session_inhibit_v1::What::HandleRebootKey as u32;

        let session_inhibit_manager::Event::Inhibit {
            session_inhibitor,
            what,
            who,
            why,
            mode,
        } = event
        else {
            return;
        };

        if what == 0 || what & !KNOWN_WHAT != 0 {
            session_inhibitor.send_error(
                session_inhibit_v1::ErrorCode::InvalidWhat,
                "what is zero or contains unknown bits",
            );
            return;
        }

        let creds = *object.client().creds();
        let uid = creds.uid.as_raw();
        let pid = creds.pid.as_raw_pid().cast_unsigned();
        let username = uzers::get_user_by_uid(uid)
            .map(|user| user.name().to_string_lossy().into_owned())
            .unwrap_or_default();
        let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
            .map(|comm| comm.trim().to_owned())
            .unwrap_or_default();
        let inhibitor_id = self.world.resource_mut::<NextInhibitorId>().take();
        let protocol_object = session_inhibitor.clone();
        let mut entity = self.world.spawn(InhibitorBundle {
            marker: Inhibitor,
            id: inhibitor_id,
            what: What(what),
            who: Who(who),
            why: Why(why),
            uid: Uid(uid),
            username: Username(username),
            pid: Pid(pid),
            comm: Comm(comm),
            mode,
            object: protocol_object,
            objects: Objects::default(),
        });

        if what & session_inhibit_v1::What::Shutdown as u32 != 0 {
            entity.insert(Shutdown);
        }
        if what & session_inhibit_v1::What::Sleep as u32 != 0 {
            entity.insert(Sleep);
        }
        if what & session_inhibit_v1::What::Idle as u32 != 0 {
            entity.insert(Idle);
        }
        if what & session_inhibit_v1::What::HandlePowerKey as u32 != 0 {
            entity.insert(HandlePowerKey);
        }
        if what & session_inhibit_v1::What::HandleSuspendKey as u32 != 0 {
            entity.insert(HandleSuspendKey);
        }
        if what & session_inhibit_v1::What::HandleHibernateKey as u32 != 0 {
            entity.insert(HandleHibernateKey);
        }
        if what & session_inhibit_v1::What::HandleLidSwitch as u32 != 0 {
            entity.insert(HandleLidKey);
        }
        if what & session_inhibit_v1::What::HandleRebootKey as u32 != 0 {
            entity.insert(HandleRebootKey);
        }

        for manager in &self.world.resource::<session::SessionInfoManagers>().0 {
            manager.send_inhibitor_added(inhibitor_id.as_raw());
        }
        session_inhibitor.send_active();
    }
}

impl hyprwire::Dispatch<session_inhibitor::SessionInhibitor> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_inhibitor::SessionInhibitor,
        event: <session_inhibitor::SessionInhibitor as hyprwire::Object>::Event<'_>,
    ) {
        if !matches!(event, session_inhibitor::Event::Destroy) {
            return;
        }
        let released = {
            let mut inhibitors = self.world.query_filtered::<(
                Entity,
                &InhibitorId,
                &session_inhibitor::SessionInhibitor,
                &Objects,
            ), With<Inhibitor>>();
            inhibitors
                .iter(&self.world)
                .find(|(_, _, candidate, _)| **candidate == *object)
                .map(|(entity, id, _, objects)| (entity, *id, objects.0.clone()))
        };

        let Some((entity, inhibitor_id, objects)) = released else {
            return;
        };

        for object in objects {
            object.send_active(0);
            object.send_destroyed();
        }
        for manager in &self.world.resource::<session::SessionInfoManagers>().0 {
            manager.send_inhibitor_removed(inhibitor_id.as_raw());
        }
        self.world.entity_mut(entity).despawn();
    }
}

#[derive(Message)]
pub struct AddObject {
    pub inhibitor_id: InhibitorId,
    pub object: session_core_v1::session_core_inhibitor::SessionCoreInhibitor,
}

const fn core_mode(mode: session_inhibit_v1::Mode) -> session_core_v1::InhibitorMode {
    match mode {
        session_inhibit_v1::Mode::BlockWeak => session_core_v1::InhibitorMode::BlockWeak,
        session_inhibit_v1::Mode::Delay => session_core_v1::InhibitorMode::Delay,
        _ => session_core_v1::InhibitorMode::Block,
    }
}

pub fn handle_add_object(
    mut requests: MessageReader<AddObject>,
    mut inhibitors: Query<
        (
            &InhibitorId,
            &What,
            &Who,
            &Why,
            &Uid,
            &Username,
            &Pid,
            &Comm,
            &session_inhibit_v1::Mode,
            &mut Objects,
        ),
        With<Inhibitor>,
    >,
) {
    for request in requests.read() {
        let Some((id, what, who, why, uid, username, pid, comm, mode, mut objects)) = inhibitors
            .iter_mut()
            .find(|(id, _, _, _, _, _, _, _, _, _)| **id == request.inhibitor_id)
        else {
            request.object.send_active(0);
            request.object.send_destroyed();
            continue;
        };

        request.object.send_inhibitor_id(id.as_raw());
        request.object.send_who(&who.0);
        request.object.send_uid(uid.0);
        request.object.send_username(&username.0);
        request.object.send_pid(pid.0);
        request.object.send_comm(&comm.0);
        request.object.send_what(what.0);
        request.object.send_why(&why.0);
        request.object.send_mode(core_mode(*mode));
        request.object.send_active(1);
        request.object.send_done();
        objects.0.push(request.object.clone());
    }
}

impl hyprwire::Dispatch<session_core_v1::session_core_inhibitor::SessionCoreInhibitor>
    for crate::Sessiond
{
    fn event(
        &mut self,
        object: &session_core_v1::session_core_inhibitor::SessionCoreInhibitor,
        event: <session_core_v1::session_core_inhibitor::SessionCoreInhibitor as hyprwire::Object>::Event<'_>,
    ) {
        if !matches!(
            event,
            session_core_v1::session_core_inhibitor::Event::Destroy
        ) {
            return;
        }
        let mut inhibitors = self.world.query::<&mut Objects>();
        for mut objects in inhibitors.iter_mut(&mut self.world) {
            objects.0.retain(|candidate| candidate != object);
        }
    }
}
