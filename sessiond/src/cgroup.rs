#[cfg(feature = "consolekit")]
use crate::consolekit;
use crate::{config, session, user};
use anyhow::Context;
use bevy_ecs::prelude::*;
use calloop::{generic, timer};
use rustix::process;
use std::collections::HashSet;
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{fs, io, path, sync, time};

#[derive(Resource)]
pub struct CgroupManager {
    root_path: path::PathBuf,
}

impl CgroupManager {
    pub fn new() -> anyhow::Result<Self> {
        let contents =
            fs::read_to_string("/proc/self/cgroup").context("failed to read /proc/self/cgroup")?;

        let cgroup_path = contents
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .context("process is not in a cgroup v2 hierarchy")?;

        let relative_path = cgroup_path
            .strip_prefix('/')
            .context("cgroup v2 path is not absolute")?;

        let process_path = path::Path::new("/sys/fs/cgroup").join(relative_path);
        let root_path = process_path
            .parent()
            .context("supervisor cgroup has no parent")?
            .to_owned();
        fs::create_dir_all(&root_path)
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

    pub(crate) fn ensure_user_cgroup(&self, uid: u32) -> anyhow::Result<path::PathBuf> {
        let user_cgroup_path = self.user_cgroup_path(uid);
        if !user_cgroup_path.exists() {
            fs::create_dir(&user_cgroup_path)?;
        }
        fs::set_permissions(&user_cgroup_path, fs::Permissions::from_mode(0o755))?;

        Ok(user_cgroup_path)
    }

    pub(crate) fn create_session_cgroup(
        &self,
        user: &uzers::User,
        session_id: session::SessionId,
        leader_pid: i32,
    ) -> anyhow::Result<CgroupBundle> {
        let user_cgroup_path = self.ensure_user_cgroup(user.uid())?;
        Self::enable_all_controllers(&self.root_path).with_context(|| {
            format!(
                "failed to enable controllers in {}",
                self.root_path.display()
            )
        })?;
        Self::enable_all_controllers(&user_cgroup_path).with_context(|| {
            format!(
                "failed to enable controllers in {}",
                user_cgroup_path.display()
            )
        })?;
        let session_cgroup_path = user_cgroup_path.join(format!("session-{}", session_id.as_raw()));
        fs::create_dir(&session_cgroup_path)?;
        fs::set_permissions(&session_cgroup_path, fs::Permissions::from_mode(0o755))?;
        fs::write(
            session_cgroup_path.join("cgroup.procs"),
            leader_pid.to_string(),
        )?;

        log::info!(session_id = session_id.as_raw(), leader_pid = leader_pid, path = session_cgroup_path.to_str(); "cgroup created");

        Ok(CgroupBundle {
            marker: Cgroup,
            session_id,
            uid: session::Uid(user.uid()),
            path: CgroupPath(session_cgroup_path),
        })
    }

    fn enable_all_controllers(parent: &path::Path) -> anyhow::Result<()> {
        let available = fs::read_to_string(parent.join("cgroup.controllers"))?;
        let available = available.split_whitespace().collect::<HashSet<_>>();

        let enabled = fs::read_to_string(parent.join("cgroup.subtree_control"))?;
        let enabled = enabled.split_whitespace().collect::<HashSet<_>>();
        let operations = available
            .iter()
            .filter(|controller| !enabled.contains(*controller))
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
            session_id,
            uid: session::Uid(uid),
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
    pub session_id: session::SessionId,
    pub uid: session::Uid,
    pub path: CgroupPath,
}

#[derive(Component)]
pub struct Cgroup;

#[derive(Component)]
pub struct CgroupPath(pub path::PathBuf);

pub fn kill(path: &CgroupPath) -> anyhow::Result<()> {
    fs::write(path.0.join("cgroup.kill"), b"1")?;
    Ok(())
}

pub fn terminate(
    path: &CgroupPath,
    session_id: session::SessionId,
    loop_handle: &calloop::LoopHandle<'static, crate::Sessiond>,
) -> anyhow::Result<()> {
    let procs = fs::read_to_string(path.0.join("cgroup.procs"))?;

    for line in procs.lines() {
        let pid = line.trim().parse::<i32>()?;
        let pid = process::Pid::from_raw(pid).context("invalid pid in cgroup.procs")?;
        process::kill_process(pid, process::Signal::TERM)?;
    }

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

#[derive(Event)]
pub struct CgroupEmpty {
    session_id: session::SessionId,
    uid: session::Uid,
}

pub fn handle_cgroup_empty(
    empty: On<CgroupEmpty>,
    mut commands: Commands,
    sessions: Query<
        (
            Entity,
            &session::SessionId,
            &session::Uid,
            &session::Gid,
            &session::Username,
            &session::Objects,
        ),
        With<session::Session>,
    >,
    mut session_registry: ResMut<session::SessionRegistry>,
    lingering_users: Query<(), (With<user::User>, With<user::Lingering>)>,
    user_registry: Res<user::UserRegistry>,
    cgroup_manager: Res<CgroupManager>,
    cgroups: Query<&CgroupPath, With<Cgroup>>,
    config: Res<config::Config>,
    shutdown_state: Res<crate::ShutdownState>,
    loop_signal: Res<crate::LoopSignal>,
    #[cfg(feature = "consolekit")] console_kit: Res<consolekit::ConsoleKit>,
) {
    let last_session = session_registry.entities_by_uid(empty.uid.0).len() == 1;

    let is_lingering = user_registry
        .by_uid
        .get(&empty.uid.0)
        .is_some_and(|&entity| lingering_users.contains(entity));

    if let Some((entity, session_id, uid, gid, username, objects)) = sessions
        .iter()
        .find(|(_, id, _, _, _, _)| **id == empty.session_id)
    {
        log::info!(
            session_id = session_id.as_raw(),
            uid = uid.0,
            username = username.0;
            "session destroyed"
        );

        for object in &objects.0 {
            object.send_destroyed();
        }

        session_registry.remove_entity(entity);
        commands.entity(entity).despawn();
        #[cfg(feature = "consolekit")]
        console_kit.remove_session(*session_id);

        let Some(user) = uzers::get_user_by_uid(uid.0) else {
            return;
        };

        let default_config = config::UserConfig::default();

        if !is_lingering
            && session_registry.entities_by_uid(uid.0).is_empty()
            && let Err(e) = user::remove_runtime_dir(
                &config
                    .users
                    .get(&sync::Arc::from(user.name().to_string_lossy()))
                    .unwrap_or(&default_config)
                    .runtime_dir(uid.0, gid.0),
            )
        {
            log::error!(uid = uid.0, error = e.to_string().as_str(); "failed to remove runtime dir");
        }
    }

    if last_session
        && !is_lingering
        && let Err(e) = cgroup_manager.remove_user_cgroup(empty.uid.0)
    {
        log::error!(
            uid = empty.uid.0,
            error = e.to_string().as_str();
            "failed to remove user cgroup"
        );
    }

    if *shutdown_state == crate::ShutdownState::Stopping && cgroups.iter().next().is_none() {
        loop_signal.0.stop();
    }
}

pub fn watch_empty(
    path: &CgroupPath,
    session_id: session::SessionId,
    uid: session::Uid,
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
                    session_id = session_id.as_raw(),
                    path = cgroup_path.to_str();
                    "cgroup destroyed"
                );

                state.world.trigger(CgroupEmpty { session_id, uid });

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
