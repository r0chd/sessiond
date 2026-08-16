use crate::session_management_v2::session_management_cgroup;
use crate::{session, user};
use anyhow::Context;
use bevy_ecs::prelude::*;
use calloop::{generic, timer};
use rustix::process;
use std::collections::HashSet;
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{fs, io, path, time};

#[derive(Resource)]
pub struct CgroupManager {
    root_path: &'static path::Path,
}

impl CgroupManager {
    pub fn new() -> anyhow::Result<Self> {
        let root_path = path::Path::new("/sys/fs/cgroup/sessiond");
        fs::create_dir_all(root_path)
            .with_context(|| format!("failed to create cgroup root at {}", root_path.display()))?;
        log::info!(root = root_path.to_str(); "cgroup manager init");

        Ok(Self { root_path })
    }

    fn user_cgroup_path(&self, uid: u32) -> path::PathBuf {
        self.root_path.join(format!("user-{uid}"))
    }

    pub(crate) fn remove_user_cgroup(&self, uid: u32) -> anyhow::Result<()> {
        let user_cgroup_path = self.user_cgroup_path(uid);

        match fs::remove_dir(user_cgroup_path) {
            Ok(()) => {
                log::info!(uid; "user cgroup destroyed");
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn remove_session_cgroup(path: &path::Path) -> anyhow::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        fs::write(path.join("cgroup.kill"), b"1")?;
        remove_cgroup_tree(path)?;
        Ok(())
    }

    pub(crate) fn ensure_user_cgroup(&self, uid: u32) -> anyhow::Result<path::PathBuf> {
        let user_cgroup_path = self.user_cgroup_path(uid);
        if !user_cgroup_path.exists() {
            fs::create_dir(&user_cgroup_path)?;
        }
        fs::set_permissions(&user_cgroup_path, fs::Permissions::from_mode(0o711))?;

        Ok(user_cgroup_path)
    }

    pub(crate) fn create_session_cgroup(
        &self,
        user: &uzers::User,
        session_id: session::SessionId,
        leader_pid: i32,
        controllers: &[String],
    ) -> anyhow::Result<CgroupBundle> {
        let user_cgroup_path = self.ensure_user_cgroup(user.uid())?;
        Self::enable_controllers(self.root_path, controllers)?;
        Self::enable_controllers(&user_cgroup_path, controllers)?;
        let session_cgroup_path = user_cgroup_path.join(format!("session-{}", session_id.as_raw()));
        fs::create_dir(&session_cgroup_path)?;
        fs::write(
            session_cgroup_path.join("cgroup.procs"),
            leader_pid.to_string(),
        )?;

        log::info!(session_id = session_id.as_raw(), leader_pid = leader_pid, path = session_cgroup_path.to_str(); "cgroup created");

        Ok(CgroupBundle {
            marker: Cgroup,
            session_id: CgroupSessionId(session_id),
            uid: CgroupUid(user.uid()),
            path: CgroupPath(session_cgroup_path),
        })
    }

    pub(crate) fn delegate_cgroup(
        user: &uzers::User,
        cgroup: &CgroupBundle,
        delegated: Delegated,
    ) -> anyhow::Result<Delegated> {
        rustix::fs::chown(
            &cgroup.path.0,
            Some(rustix::process::Uid::from_raw(user.uid())),
            Some(rustix::process::Gid::from_raw(user.primary_group_id())),
        )?;
        fs::set_permissions(&cgroup.path.0, fs::Permissions::from_mode(0o700))?;
        for name in ["cgroup.procs", "cgroup.threads", "cgroup.subtree_control"] {
            let file = cgroup.path.0.join(name);
            rustix::fs::chown(
                &file,
                Some(rustix::process::Uid::from_raw(user.uid())),
                Some(rustix::process::Gid::from_raw(user.primary_group_id())),
            )?;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600))?;
        }
        delegated.object.send_path(cgroup.path.0.to_string_lossy());
        Ok(delegated)
    }

    fn enable_controllers(parent: &path::Path, requested: &[String]) -> anyhow::Result<()> {
        if requested.is_empty() {
            return Ok(());
        }

        let available = fs::read_to_string(parent.join("cgroup.controllers"))?;
        let available = available.split_whitespace().collect::<HashSet<_>>();
        for controller in requested {
            if !available.contains(controller.as_str()) {
                anyhow::bail!(
                    "controller {controller} is unavailable in {}",
                    parent.display()
                );
            }
        }

        let enabled = fs::read_to_string(parent.join("cgroup.subtree_control"))?;
        let enabled = enabled.split_whitespace().collect::<HashSet<_>>();
        let operations = requested
            .iter()
            .filter(|controller| !enabled.contains(controller.as_str()))
            .map(|controller| format!("+{controller}"))
            .collect::<Vec<_>>();

        if !operations.is_empty() {
            fs::write(parent.join("cgroup.subtree_control"), operations.join(" "))?;
        }

        Ok(())
    }

    pub(crate) fn restore_session_cgroup(
        &self,
        uid: u32,
        session_id: session::SessionId,
    ) -> anyhow::Result<CgroupBundle> {
        let user_cgroup_path = self.user_cgroup_path(uid);
        let session_cgroup_path = user_cgroup_path.join(format!("session-{}", session_id.as_raw()));
        if !session_cgroup_path.exists() {
            anyhow::bail!("session cgroup does not exist");
        }

        let events = fs::read_to_string(session_cgroup_path.join("cgroup.events"))?;
        if events.contains("populated 0") {
            anyhow::bail!("session cgroup is empty");
        }

        Ok(CgroupBundle {
            marker: Cgroup,
            session_id: CgroupSessionId(session_id),
            uid: CgroupUid(uid),
            path: CgroupPath(session_cgroup_path),
        })
    }

    pub(crate) fn find_session_by_pid(
        &self,
        pid: u32,
    ) -> anyhow::Result<Option<session::SessionId>> {
        _ = self;
        let contents = fs::read_to_string(format!("/proc/{pid}/cgroup"))?;

        let session_id = contents.lines().find_map(|line| {
            let path = line.rsplit_once(':')?.1;
            path.split('/')
                .find_map(|component| component.strip_prefix("session-")?.parse().ok())
        });

        Ok(session_id.map(session::SessionId::from_raw))
    }
}

fn remove_cgroup_tree(path: &path::Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            remove_cgroup_tree(&child)?;
        }
    }
    fs::remove_dir(path)?;
    Ok(())
}

#[derive(Bundle)]
pub struct CgroupBundle {
    pub marker: Cgroup,
    pub session_id: CgroupSessionId,
    pub uid: CgroupUid,
    pub path: CgroupPath,
}

#[derive(Component)]
pub struct Cgroup;

#[derive(Component, Clone, Copy)]
pub struct CgroupSessionId(pub session::SessionId);

#[derive(Component, Clone, Copy)]
pub struct CgroupUid(pub u32);

#[derive(Component)]
pub struct CgroupPath(pub path::PathBuf);

#[derive(Component, Clone)]
pub struct Delegated {
    pub object: session_management_cgroup::SessionManagementCgroup,
}

pub fn kill(
    path: &CgroupPath,
    session_id: CgroupSessionId,
    uid: CgroupUid,
    loop_handle: &calloop::LoopHandle<'static, crate::Sessiond>,
) -> anyhow::Result<()> {
    fs::write(path.0.join("cgroup.kill"), b"1")?;
    poll_destroy(path, session_id, uid, loop_handle)?;
    Ok(())
}

pub fn terminate(
    path: &CgroupPath,
    session_id: CgroupSessionId,
    uid: CgroupUid,
    loop_handle: &calloop::LoopHandle<'static, crate::Sessiond>,
) -> anyhow::Result<()> {
    let procs = fs::read_to_string(path.0.join("cgroup.procs"))?;

    for line in procs.lines() {
        let pid = line.trim().parse::<i32>()?;
        let pid = process::Pid::from_raw(pid).context("invalid pid in cgroup.procs")?;
        process::kill_process(pid, process::Signal::TERM)?;
    }

    poll_destroy(path, session_id, uid, loop_handle)?;

    let session_id = session_id.0;
    let path = path.0.clone();
    loop_handle
        .insert_source(
            timer::Timer::from_duration(time::Duration::from_secs(5)),
            move |_, (), _| {
                if path.exists()
                    && let Err(err) = fs::write(path.join("cgroup.kill"), b"1")
                {
                    log::error!(
                        session_id = session_id.as_raw(),
                        error = err.to_string().as_str();
                        "failed to force-terminate session"
                    );
                }
                timer::TimeoutAction::Drop
            },
        )
        .map_err(|err| anyhow::anyhow!("failed to schedule termination fallback: {err:?}"))?;
    Ok(())
}

fn poll_destroy(
    path: &CgroupPath,
    session_id: CgroupSessionId,
    uid: CgroupUid,
    loop_handle: &calloop::LoopHandle<'static, crate::Sessiond>,
) -> anyhow::Result<()> {
    let cgroup_path = path.0.clone();
    let mut file = fs::File::open(path.0.join("cgroup.events"))?;
    let fd_wrapper = unsafe { generic::FdWrapper::new(file.as_raw_fd()) };
    let source = generic::Generic::new(
        fd_wrapper,
        calloop::Interest {
            readable: true,
            writable: false,
        },
        calloop::Mode::Level,
    );

    loop_handle.insert_source(source, move |_, _, state| {
        let result = (|| -> anyhow::Result<calloop::PostAction> {
            file.seek(io::SeekFrom::Start(0))?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;

            if contents.contains("populated 0") {
                remove_cgroup_tree(&cgroup_path)?;
                log::info!(
                    session_id = session_id.0.as_raw(),
                    path = cgroup_path.to_str();
                    "cgroup destroyed"
                );

                let last_session = state
                    .world
                    .resource::<session::SessionRegistry>()
                    .entities_by_uid(uid.0)
                    .len()
                    == 1;

                state.finish_session_destroy(session_id.0);

                if last_session
                    && !user::is_lingering(&state.world, uid.0)
                    && let Err(e) = state
                        .world
                        .resource::<CgroupManager>()
                        .remove_user_cgroup(uid.0)
                {
                    log::error!(
                        uid = uid.0,
                        error = e.to_string().as_str();
                        "failed to remove user cgroup"
                    );
                }

                let mut cgroups = state.world.query::<&Cgroup>();
                if state.shutting_down && cgroups.iter(&state.world).next().is_none() {
                    state.loop_signal.stop();
                }

                return Ok(calloop::PostAction::Remove);
            }

            Ok(calloop::PostAction::Continue)
        })();

        result.or_else(|error| {
            log::error!(error = error.to_string().as_str(); "cgroup poll error");
            Ok(calloop::PostAction::Remove)
        })
    })?;
    Ok(())
}

impl hyprwire::Dispatch<session_management_cgroup::SessionManagementCgroup> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_management_cgroup::SessionManagementCgroup,
        event: <session_management_cgroup::SessionManagementCgroup as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_management_cgroup::Event::Controller { name } => {
                let error = {
                    let mut pending_sessions =
                        self.world.resource_mut::<session::PendingSessions>();
                    let pending =
                        pending_sessions
                            .by_management_object
                            .values_mut()
                            .find(|pending| {
                                pending
                                    .delegated_cgroup
                                    .as_ref()
                                    .is_some_and(|delegated| delegated.object == *object)
                            });
                    let Some(pending) = pending else {
                        return object.send_error(
                            crate::session_management_v2::ErrorCode::NotFound,
                            "delegated cgroup is no longer pending",
                        );
                    };
                    if pending.delegated_committed {
                        Some("delegated cgroup configuration is already committed")
                    } else {
                        pending.delegated_controllers.push(name);
                        None
                    }
                };
                if let Some(error) = error {
                    object.send_error(crate::session_management_v2::ErrorCode::Io, error);
                }
            }
            session_management_cgroup::Event::Commit => {
                let mut pending_sessions = self.world.resource_mut::<session::PendingSessions>();
                let pending = pending_sessions
                    .by_management_object
                    .values_mut()
                    .find(|pending| {
                        pending
                            .delegated_cgroup
                            .as_ref()
                            .is_some_and(|delegated| delegated.object == *object)
                    });
                let Some(pending) = pending else {
                    object.send_error(
                        crate::session_management_v2::ErrorCode::NotFound,
                        "delegated cgroup is no longer pending",
                    );
                    return;
                };
                if pending.delegated_committed {
                    object.send_error(
                        crate::session_management_v2::ErrorCode::Io,
                        "delegated cgroup is already committed",
                    );
                } else {
                    pending.delegated_committed = true;
                }
            }
            session_management_cgroup::Event::Destroy => {
                if self
                    .world
                    .resource_mut::<session::PendingSessions>()
                    .by_management_object
                    .values_mut()
                    .find(|pending| {
                        pending
                            .delegated_cgroup
                            .as_ref()
                            .is_some_and(|delegated| delegated.object == *object)
                    })
                    .map(|pending| {
                        pending.delegated_cgroup = None;
                        pending.delegated_controllers.clear();
                    })
                    .is_some()
                {
                    return;
                }

                let mut query = self.world.query::<(Entity, &Cgroup, &Delegated)>();
                let entity = query.iter(&self.world).find_map(|(entity, _, delegated)| {
                    (delegated.object == *object).then_some(entity)
                });

                if let Some(entity) = entity {
                    self.kill_session(entity);
                }
            }
        }
    }
}
