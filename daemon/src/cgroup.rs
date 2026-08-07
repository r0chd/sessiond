use crate::{session, user};
use anyhow::Context;
use calloop::{generic, timer};
use rustix::process;
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::{fs, io, path, time};

pub struct CgroupManager {
    root_path: Option<&'static path::Path>,
    cgroups: Vec<Cgroup>,
    loop_handle: calloop::LoopHandle<'static, crate::Sessiond>,
}

impl CgroupManager {
    pub fn new(loop_handle: calloop::LoopHandle<'static, crate::Sessiond>) -> anyhow::Result<Self> {
        let root_path = path::Path::new("/sys/fs/cgroup/sessiond");
        fs::create_dir_all(root_path)
            .with_context(|| format!("failed to create cgroup root at {}", root_path.display()))?;
        log::info!(root = root_path.to_str(); "cgroup manager init");

        Ok(Self {
            root_path: Some(root_path),
            cgroups: Vec::new(),
            loop_handle,
        })
    }

    fn user_cgroup_path(&self, uid: u32) -> anyhow::Result<path::PathBuf> {
        Ok(self
            .root_path
            .context("cgroup manager is disabled")?
            .join(format!("user-{uid}")))
    }

    pub(crate) fn remove_user_cgroup(&self, uid: u32) -> anyhow::Result<()> {
        let Ok(user_cgroup_path) = self.user_cgroup_path(uid) else {
            return Ok(());
        };

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
        if self.root_path.is_none() {
            return Ok(path::PathBuf::new());
        }

        let user_cgroup_path = self.user_cgroup_path(uid)?;
        if !user_cgroup_path.exists() {
            fs::create_dir(&user_cgroup_path)?;
        }

        Ok(user_cgroup_path)
    }

    pub(crate) fn create_session_cgroup(
        &mut self,
        uid: u32,
        session_id: session::SessionId,
        leader_pid: i32,
    ) -> anyhow::Result<()> {
        if self.root_path.is_none() {
            return Ok(());
        }

        let user_cgroup_path = self.ensure_user_cgroup(uid)?;
        let session_cgroup_path = user_cgroup_path.join(format!("session-{}", session_id.as_raw()));
        fs::create_dir(&session_cgroup_path)?;
        fs::write(
            session_cgroup_path.join("cgroup.procs"),
            leader_pid.to_string(),
        )?;

        log::info!(session_id = session_id.as_raw(), leader_pid = leader_pid, path = session_cgroup_path.to_str(); "cgroup created");

        self.cgroups.push(Cgroup {
            session_id,
            uid,
            path: session_cgroup_path,
        });

        Ok(())
    }

    pub(crate) fn restore_session_cgroup(
        &mut self,
        uid: u32,
        session_id: session::SessionId,
    ) -> anyhow::Result<()> {
        let user_cgroup_path = self.user_cgroup_path(uid)?;
        let session_cgroup_path = user_cgroup_path.join(format!("session-{}", session_id.as_raw()));
        if !session_cgroup_path.exists() {
            anyhow::bail!("session cgroup does not exist");
        }

        let events = fs::read_to_string(session_cgroup_path.join("cgroup.events"))?;
        if events.contains("populated 0") {
            anyhow::bail!("session cgroup is empty");
        }

        self.cgroups.push(Cgroup {
            session_id,
            uid,
            path: session_cgroup_path,
        });

        Ok(())
    }

    fn poll_destroy(&self, session_id: session::SessionId) -> anyhow::Result<()> {
        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        let mut file = fs::File::open(cgroup.path.join("cgroup.events"))?;

        let fd_wrapper = unsafe { generic::FdWrapper::new(file.as_raw_fd()) };
        let source = generic::Generic::new(
            fd_wrapper,
            calloop::Interest {
                readable: true,
                writable: false,
            },
            calloop::Mode::Level,
        );
        self.loop_handle.insert_source(source, move |_, _, state| {
            let result = (|| -> anyhow::Result<calloop::PostAction> {
                file.seek(io::SeekFrom::Start(0))?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;

                if contents.contains("populated 0") {
                    let Some((uid, cgroup_path)) = state.cgroup_manager.take_cgroup(session_id)
                    else {
                        return Ok(calloop::PostAction::Remove);
                    };

                    fs::remove_dir(&cgroup_path)?;
                    log::info!(
                        session_id = session_id.as_raw(),
                        path = cgroup_path.to_str();
                        "cgroup destroyed"
                    );

                    if !state.cgroup_manager.has_uid_sessions(uid)
                        && !user::is_lingering(&state.world, uid)
                        && let Err(e) = state.cgroup_manager.remove_user_cgroup(uid)
                    {
                        log::error!(
                            uid = uid,
                            error = e.to_string().as_str();
                            "failed to remove user cgroup"
                        );
                    }

                    state.cleanup_user_runtime_dir(uid);

                    if state.shutting_down && state.cgroup_manager.is_empty() {
                        state.loop_signal.stop();
                    }

                    return Ok(calloop::PostAction::Remove);
                }

                Ok(calloop::PostAction::Continue)
            })();

            result.or_else(|e| {
                log::error!(
                    session_id = session_id.as_raw(),
                    error = e.to_string().as_str();
                    "cgroup poll error"
                );

                Ok(calloop::PostAction::Remove)
            })
        })?;

        Ok(())
    }

    pub(crate) fn kill_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
        if self.root_path.is_none() {
            return Ok(());
        }

        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        fs::write(cgroup.path.join("cgroup.kill"), b"1")?;
        self.poll_destroy(session_id)?;
        Ok(())
    }

    pub(crate) fn terminate_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
        if self.root_path.is_none() {
            return Ok(());
        }

        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        let procs = fs::read_to_string(cgroup.path.join("cgroup.procs"))?;

        for line in procs.lines() {
            let pid = line.trim().parse::<i32>()?;
            let pid = process::Pid::from_raw(pid).context("invalid pid in cgroup.procs")?;
            process::kill_process(pid, process::Signal::TERM)?;
        }

        self.poll_destroy(session_id)?;

        let path = cgroup.path.clone();
        self.loop_handle
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
            .map_err(|err| {
                anyhow::anyhow!("failed to schedule session termination fallback: {err:?}")
            })?;
        Ok(())
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

    fn take_cgroup(&mut self, session_id: session::SessionId) -> Option<(u32, path::PathBuf)> {
        self.cgroups
            .extract_if(.., |c| c.session_id == session_id)
            .next()
            .map(|c| (c.uid, c.path))
    }

    pub(crate) fn has_uid_sessions(&self, uid: u32) -> bool {
        self.cgroups.iter().any(|c| c.uid == uid)
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.cgroups.is_empty()
    }
}

struct Cgroup {
    session_id: session::SessionId,
    uid: u32,
    path: path::PathBuf,
}
