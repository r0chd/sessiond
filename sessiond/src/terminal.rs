use rustix::ioctl;
use rustix::process;
use std::{fs, path};

#[allow(clippy::struct_field_names)]
#[repr(C)]
struct VtStat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}
#[repr(C)]
struct VtMode {
    mode: u8,
    waitv: u8,
    relsig: i16,
    acqsig: i16,
    frsig: i16,
}

const VT_GETSTATE: ioctl::Opcode = 0x5603;
const VT_SETMODE: ioctl::Opcode = 0x5602;
const VT_RELDISP: ioctl::Opcode = 0x5605;
const VT_AUTO: u8 = 0;
const VT_PROCESS: u8 = 1;
const VT_ACKACQ: usize = 2;

pub struct Terminal(fs::File);

impl Terminal {
    fn tty_path(tty: u32) -> path::PathBuf {
        path::PathBuf::from(format!("/dev/tty{tty}"))
    }

    pub fn open(tty: u32) -> anyhow::Result<Self> {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(Self::tty_path(tty))?;
        Ok(Self(file))
    }

    pub fn current_vt(&self) -> anyhow::Result<u16> {
        let state = unsafe { ioctl::ioctl(&self.0, ioctl::Getter::<VT_GETSTATE, VtStat>::new())? };
        Ok(state.v_active)
    }

    pub fn set_process_switching(&self, enable: bool) -> anyhow::Result<()> {
        let mode = VtMode {
            mode: if enable { VT_PROCESS } else { VT_AUTO },
            waitv: 0,
            relsig: if enable {
                i16::try_from(process::Signal::USR1.as_raw())?
            } else {
                0
            },
            acqsig: if enable {
                i16::try_from(process::Signal::USR2.as_raw())?
            } else {
                0
            },
            frsig: 0,
        };

        unsafe { ioctl::ioctl(&self.0, ioctl::Setter::<VT_SETMODE, VtMode>::new(mode))? };
        Ok(())
    }

    pub fn ack_release(&self) -> anyhow::Result<()> {
        unsafe { ioctl::ioctl(&self.0, ioctl::IntegerSetter::<VT_RELDISP>::new_usize(1))? };
        Ok(())
    }

    pub fn ack_acquire(&self) -> anyhow::Result<()> {
        unsafe {
            ioctl::ioctl(
                &self.0,
                ioctl::IntegerSetter::<VT_RELDISP>::new_usize(VT_ACKACQ),
            )?;
        };
        Ok(())
    }
}
