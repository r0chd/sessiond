use crate::session::SessionId;
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

    pub fn create_session_cgroup(
        &mut self,
        session_id: SessionId,
        leader_pid: i32,
    ) -> anyhow::Result<()> {
        let session_cgroup_path = self
            .root_path
            .join(format!("session-{}", session_id.as_raw()));
        fs::create_dir(&session_cgroup_path)?;
        fs::write(
            session_cgroup_path.join("cgroup.procs"),
            leader_pid.to_string(),
        )?;

        log::info!(session_id = session_id.as_raw(), leader_pid = leader_pid, path = session_cgroup_path.to_str(); "cgroup created");

        let cgroup = Cgroup {
            session_id,
            path: session_cgroup_path,
        };

        self.cgroups.push(cgroup);

        Ok(())
    }

    pub fn poll_destroy(&self, session_id: SessionId) -> anyhow::Result<()> {
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
                    return Ok(calloop::PostAction::Remove);
                }

                Ok(calloop::PostAction::Continue)
            })();

            match result {
                Ok(action) => Ok(action),
                Err(e) => {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "cgroup poll error");
                    Ok(calloop::PostAction::Remove)
                }
            }
        })?;

        Ok(())
    }

    pub fn kill_session(&self, session_id: SessionId) -> anyhow::Result<()> {
        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        fs::write(cgroup.path.join("cgroup.kill"), b"1")?;

        self.poll_destroy(session_id)?;

        Ok(())
    }

    pub fn terminate_session(&self, session_id: SessionId) -> anyhow::Result<()> {
        let cgroup = self
            .cgroups
            .iter()
            .find(|cgroup| cgroup.session_id == session_id)
            .context("session not found")?;
        let procs = fs::read_to_string(cgroup.path.join("cgroup.procs"))?;
        for line in procs.lines() {
            let pid = line.trim().parse::<i32>()?;

            process::kill_process(process::Pid::from_raw(pid).unwrap(), process::Signal::TERM)?;
        }

        self.poll_destroy(session_id)?;

        Ok(())
    }

    pub fn find_session_by_pid(&self, pid: u32) -> anyhow::Result<Option<SessionId>> {
        for cgroup in self.cgroups.iter() {
            let procs = fs::read_to_string(cgroup.path.join("cgroup.procs"))?;

            if procs
                .lines()
                .find(|line| *line.trim() == pid.to_string())
                .is_some()
            {
                return Ok(Some(cgroup.session_id));
            }
        }

        Ok(None)
    }
}

struct Cgroup {
    session_id: SessionId,
    path: path::PathBuf,
}
