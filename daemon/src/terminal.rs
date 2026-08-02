use rustix::ioctl;
use std::fs::File;
use std::path;

#[repr(C)]
struct VtStat {
    _v_active: u16,
    v_signal: u16,
    v_state: u16,
}
const VT_GETSTATE: ioctl::Opcode = 0x5603;

pub struct Terminal(File);

impl Terminal {
    fn tty_path(tty: u32) -> path::PathBuf {
        path::PathBuf::from(format!("/dev/tty{tty}"))
    }

    pub fn open(tty: u32) -> anyhow::Result<Self> {
        let file = File::open(Self::tty_path(tty))?;
        Ok(Self(file))
    }

    pub fn current_vt(&self) -> anyhow::Result<u16> {
        let state = unsafe { ioctl::ioctl(&self.0, ioctl::Getter::<VT_GETSTATE, VtStat>::new())? };
        Ok(state._v_active)
    }
}
