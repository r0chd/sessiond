use bevy_ecs::prelude::Resource;
use rustix::process;
use std::fs;

#[derive(Clone, Copy)]
pub struct Access {
    pub user: process::Uid,
    pub group: process::Gid,
}

impl Access {
    pub const fn new(user: process::Uid, group: process::Gid) -> Self {
        Self { user, group }
    }

    pub fn allows(self, creds: &hyprwire::Creds) -> bool {
        if self.user == creds.uid || self.group == creds.gid || creds.uid.is_root() {
            return true;
        }

        Self::process_groups(creds.pid.as_raw_pid().cast_unsigned())
            .is_ok_and(|groups| groups.contains(&self.group))
    }

    fn process_groups(pid: u32) -> anyhow::Result<Vec<process::Gid>> {
        let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
        let Some(groups) = status.lines().find_map(|line| line.strip_prefix("Groups:")) else {
            return Ok(Vec::new());
        };

        groups
            .split_whitespace()
            .map(|group| {
                group
                    .parse::<process::RawGid>()
                    .map(process::Gid::from_raw)
                    .map_err(Into::into)
            })
            .collect()
    }
}

#[derive(Resource)]
pub struct AccessRules {
    pub management: Access,
}
