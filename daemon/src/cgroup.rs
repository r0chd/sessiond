use crate::{session, user};
use anyhow::Context;
use calloop::generic;
use rustix::process;
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::{fs, io, path};

pub trait CgroupManager {
    fn ensure_user_cgroup(&self, uid: u32) -> anyhow::Result<path::PathBuf>;
    fn remove_user_cgroup(&self, uid: u32) -> anyhow::Result<()>;
    fn create_session_cgroup(
        &mut self,
        uid: u32,
        session_id: session::SessionId,
        leader_pid: i32,
    ) -> anyhow::Result<()>;
    fn poll_destroy(&self, session_id: session::SessionId) -> anyhow::Result<()>;
    fn kill_session(&self, session_id: session::SessionId) -> anyhow::Result<()>;
    fn terminate_session(&self, session_id: session::SessionId) -> anyhow::Result<()>;
    fn find_session_by_pid(&self, pid: u32) -> anyhow::Result<Option<session::SessionId>>;
    fn take_cgroup(&mut self, session_id: session::SessionId) -> Option<(u32, path::PathBuf)>;
    fn has_uid_sessions(&self, uid: u32) -> bool;
    fn is_empty(&self) -> bool;
}

pub struct SystemCgroupManager {
    root_path: &'static path::Path,
    cgroups: Vec<Cgroup>,
    loop_handle: calloop::LoopHandle<'static, crate::Sessiond<Self>>,
}

impl SystemCgroupManager {
    pub fn new(
        loop_handle: calloop::LoopHandle<'static, crate::Sessiond<Self>>,
    ) -> anyhow::Result<Self> {
        let root_path = path::Path::new("/sys/fs/cgroup/sessiond");
        fs::create_dir_all(root_path)
            .with_context(|| format!("failed to create cgroup root at {}", root_path.display()))?;
        log::info!(root = root_path.to_str(); "cgroup manager init");

        Ok(Self {
            root_path,
            cgroups: Vec::new(),
            loop_handle,
        })
    }

    fn user_cgroup_path(&self, uid: u32) -> path::PathBuf {
        self.root_path.join(format!("user-{uid}"))
    }
}

impl CgroupManager for SystemCgroupManager {
    fn remove_user_cgroup(&self, uid: u32) -> anyhow::Result<()> {
        match fs::remove_dir(self.user_cgroup_path(uid)) {
            Ok(()) => {
                log::info!(uid; "user cgroup destroyed");
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn ensure_user_cgroup(&self, uid: u32) -> anyhow::Result<path::PathBuf> {
        let user_cgroup_path = self.user_cgroup_path(uid);
        if !user_cgroup_path.exists() {
            fs::create_dir(&user_cgroup_path)?;
        }

        Ok(user_cgroup_path)
    }

    fn create_session_cgroup(
        &mut self,
        uid: u32,
        session_id: session::SessionId,
        leader_pid: i32,
    ) -> anyhow::Result<()> {
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
                    let Some((uid, cgroup_path)) =
                        state.cgroup_manager.take_cgroup(session_id)
                    else {
                        return Ok(calloop::PostAction::Remove);
                    };

                    fs::remove_dir(&cgroup_path)?;
                    log::info!(session_id = session_id.as_raw(), path = cgroup_path.to_str(); "cgroup destroyed");

                    if !state.cgroup_manager.has_uid_sessions(uid)
                        && !user::is_lingering(&state.world, &state.user_entities, uid)
                        && let Err(e) = state.cgroup_manager.remove_user_cgroup(uid)
                    {
                        log::error!(uid = uid, error = e.to_string().as_str(); "failed to remove user cgroup");
                    }

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

    fn kill_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        fs::write(cgroup.path.join("cgroup.kill"), b"1")?;
        self.poll_destroy(session_id)?;
        Ok(())
    }

    fn terminate_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn find_session_by_pid(&self, pid: u32) -> anyhow::Result<Option<session::SessionId>> {
        for cgroup in &self.cgroups {
            let procs = fs::read_to_string(cgroup.path.join("cgroup.procs"))?;

            if procs
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .any(|cgroup_pid| cgroup_pid == pid)
            {
                return Ok(Some(cgroup.session_id));
            }
        }

        Ok(None)
    }

    fn take_cgroup(&mut self, session_id: session::SessionId) -> Option<(u32, path::PathBuf)> {
        self.cgroups
            .extract_if(.., |c| c.session_id == session_id)
            .next()
            .map(|c| (c.uid, c.path))
    }

    fn has_uid_sessions(&self, uid: u32) -> bool {
        self.cgroups.iter().any(|c| c.uid == uid)
    }

    fn is_empty(&self) -> bool {
        self.cgroups.is_empty()
    }
}

pub struct DummyCgroupManager;

impl CgroupManager for DummyCgroupManager {
    fn ensure_user_cgroup(&self, _uid: u32) -> anyhow::Result<path::PathBuf> {
        Ok(path::PathBuf::new())
    }

    fn remove_user_cgroup(&self, _uid: u32) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_session_cgroup(
        &mut self,
        _uid: u32,
        _session_id: session::SessionId,
        _leader_pid: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn poll_destroy(&self, _session_id: session::SessionId) -> anyhow::Result<()> {
        Ok(())
    }

    fn kill_session(&self, _session_id: session::SessionId) -> anyhow::Result<()> {
        Ok(())
    }

    fn terminate_session(&self, _session_id: session::SessionId) -> anyhow::Result<()> {
        Ok(())
    }

    fn find_session_by_pid(&self, _pid: u32) -> anyhow::Result<Option<session::SessionId>> {
        Ok(None)
    }

    fn take_cgroup(&mut self, _session_id: session::SessionId) -> Option<(u32, path::PathBuf)> {
        None
    }

    fn has_uid_sessions(&self, _uid: u32) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        true
    }
}

struct Cgroup {
    session_id: session::SessionId,
    uid: u32,
    path: path::PathBuf,
}
