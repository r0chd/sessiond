use rustix::ioctl;
use std::{fs, path};

#[allow(clippy::struct_field_names)]
#[repr(C)]
struct VtStat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}
const VT_GETSTATE: ioctl::Opcode = 0x5603;

pub struct Terminal(fs::File);

impl Terminal {
    fn tty_path(tty: u32) -> path::PathBuf {
        path::PathBuf::from(format!("/dev/tty{tty}"))
    }

    pub fn open(tty: u32) -> anyhow::Result<Self> {
        let file = fs::File::open(Self::tty_path(tty))?;
        Ok(Self(file))
    }

    pub fn current_vt(&self) -> anyhow::Result<u16> {
        let state = unsafe { ioctl::ioctl(&self.0, ioctl::Getter::<VT_GETSTATE, VtStat>::new())? };
        Ok(state.v_active)
    }
}
