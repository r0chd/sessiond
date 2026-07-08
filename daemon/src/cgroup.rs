use crate::{session, user};
use anyhow::Context;
use calloop::generic;
use rustix::process;
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::{fs, io, path};

pub struct CgroupManager {
    root_path: &'static path::Path,
    cgroups: Vec<Cgroup>,
    loop_handle: calloop::LoopHandle<'static, crate::Sessiond>,
}

impl CgroupManager {
    pub fn new(loop_handle: calloop::LoopHandle<'static, crate::Sessiond>) -> anyhow::Result<Self> {
        let root_path = path::Path::new("/sys/fs/cgroup/sessiond");
        fs::create_dir_all(root_path)?;
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

    pub fn remove_user_cgroup(&self, uid: u32) -> anyhow::Result<()> {
        match fs::remove_dir(self.user_cgroup_path(uid)) {
            Ok(()) => {
                log::info!(uid; "user cgroup destroyed");
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn ensure_user_cgroup(&self, uid: u32) -> anyhow::Result<path::PathBuf> {
        let user_cgroup_path = self.user_cgroup_path(uid);
        if !user_cgroup_path.exists() {
            fs::create_dir(&user_cgroup_path)?;
        }

        Ok(user_cgroup_path)
    }

    pub fn create_session_cgroup(
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

        let cgroup = Cgroup {
            session_id,
            uid,
            path: session_cgroup_path,
        };

        self.cgroups.push(cgroup);

        Ok(())
    }

    pub fn poll_destroy(&self, session_id: session::SessionId) -> anyhow::Result<()> {
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
                    let cgroup = state
                        .cgroup_manager
                        .cgroups
                        .extract_if(.., |cgroup| cgroup.session_id == session_id)
                        .next()
                        .context("session cgroup not found")?;

                    fs::remove_dir(&cgroup.path)?;
                    log::info!(session_id = session_id.as_raw(), path = cgroup.path.to_str(); "cgroup destroyed");

                    let has_other_sessions = state
                        .cgroup_manager
                        .cgroups
                        .iter()
                        .any(|other| other.uid == cgroup.uid);
                    if !has_other_sessions
                        && !user::is_lingering(&state.world, &state.user_entities, cgroup.uid)
                        && let Err(e) = state.cgroup_manager.remove_user_cgroup(cgroup.uid)
                    {
                        log::error!(uid = cgroup.uid, error = e.to_string().as_str(); "failed to remove user cgroup");
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

    pub fn kill_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        fs::write(cgroup.path.join("cgroup.kill"), b"1")?;

        self.poll_destroy(session_id)?;

        Ok(())
    }

    pub fn terminate_session(&self, session_id: session::SessionId) -> anyhow::Result<()> {
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

    pub fn find_session_by_pid(&self, pid: u32) -> anyhow::Result<Option<session::SessionId>> {
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

    pub const fn is_empty(&self) -> bool {
        self.cgroups.is_empty()
    }
}

struct Cgroup {
    session_id: session::SessionId,
    uid: u32,
    path: path::PathBuf,
}
