use rustix::ioctl;
use rustix::process;
use std::{fs, path};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct VtStat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct VtMode {
    mode: u8,
    waitv: u8,
    relsig: i16,
    acqsig: i16,
    frsig: i16,
}

const VT_GETSTATE: ioctl::Opcode = 0x5603;
const VT_SETMODE: ioctl::Opcode = 0x5602;
const VT_ACTIVATE: ioctl::Opcode = 0x5606;
const VT_RELDISP: ioctl::Opcode = 0x5605;
const KDSETMODE: ioctl::Opcode = 0x4B3A;
const KDSKBMODE: ioctl::Opcode = 0x4B45;
const VT_AUTO: u8 = 0x00;
const VT_PROCESS: u8 = 0x01;
const VT_ACKACQ: i32 = 0x02;
const KD_TEXT: i32 = 0x00;
const KD_GRAPHICS: i32 = 0x01;
const K_UNICODE: i32 = 0x03;
const K_OFF: i32 = 0x04;
const FRSIG: i16 = 0;

pub struct Terminal(fs::File);

impl Terminal {
    fn get_tty_path(tty: u32) -> path::PathBuf {
        path::PathBuf::from(format!("/dev/tty{tty}"))
    }

    pub fn open(tty: u32) -> anyhow::Result<Self> {
        let path = Self::get_tty_path(tty);
        Ok(Self(fs::File::open(&path)?))
    }

    pub fn current_vt(&self) -> anyhow::Result<u16> {
        let st = unsafe { ioctl::ioctl(&self.0, ioctl::Getter::<VT_GETSTATE, VtStat>::new())? };

        Ok(st.v_active)
    }

    pub fn set_process_switching(&self, enable: bool) -> anyhow::Result<()> {
        log::debug!("Setting process switching to {enable}");

        let mode = VtMode {
            mode: if enable { VT_PROCESS } else { VT_AUTO },
            waitv: 0,
            relsig: if enable {
                process::Signal::USR1.as_raw() as i16
            } else {
                0
            },
            acqsig: if enable {
                process::Signal::USR2.as_raw() as i16
            } else {
                0
            },
            frsig: FRSIG,
        };

        let result =
            unsafe { ioctl::ioctl(&self.0, ioctl::Setter::<VT_SETMODE, VtMode>::new(mode)) };

        if let Err(err) = result {
            log::error!(
                "Could not set VT mode to {} process switching: {}",
                if enable { "enable" } else { "disable" },
                err
            );
            return Err(err.into());
        }

        Ok(())
    }

    pub fn switch_vt(&self, vt: i32) -> anyhow::Result<()> {
        log::debug!("Switching to VT {vt}");

        let result = unsafe {
            ioctl::ioctl(
                &self.0,
                ioctl::IntegerSetter::<VT_ACTIVATE>::new_usize(vt as usize),
            )
        };

        if let Err(err) = result {
            log::error!("Could not activate VT {vt}: {err}");
            return Err(err.into());
        }

        Ok(())
    }

    pub fn ack_release(&self) -> anyhow::Result<()> {
        log::debug!("Acking VT release");

        let result =
            unsafe { ioctl::ioctl(&self.0, ioctl::IntegerSetter::<VT_RELDISP>::new_usize(1)) };

        if let Err(err) = result {
            log::error!("Could not ack VT release: {err}");
            return Err(err.into());
        }

        Ok(())
    }

    pub fn ack_acquire(&self) -> anyhow::Result<()> {
        log::debug!("Acking VT acquire");

        let result = unsafe {
            ioctl::ioctl(
                &self.0,
                ioctl::IntegerSetter::<VT_RELDISP>::new_usize(VT_ACKACQ as usize),
            )
        };

        if let Err(err) = result {
            log::error!("Could not ack VT acquire: {err}");
            return Err(err.into());
        }

        Ok(())
    }

    pub fn set_keyboard(&self, enable: bool) -> anyhow::Result<()> {
        log::debug!("Setting KD keyboard state to {enable}");

        let result = unsafe {
            ioctl::ioctl(
                &self.0,
                ioctl::IntegerSetter::<KDSKBMODE>::new_usize(
                    (if enable { K_UNICODE } else { K_OFF }) as usize,
                ),
            )
        };

        if let Err(err) = result {
            log::error!(
                "Could not set KD keyboard mode to {}: {}",
                if enable { "enabled" } else { "disabled" },
                err
            );
            return Err(err.into());
        }

        Ok(())
    }

    pub fn set_graphics(&self, enable: bool) -> anyhow::Result<()> {
        log::debug!("Setting KD graphics state to {enable}");

        let result = unsafe {
            ioctl::ioctl(
                &self.0,
                ioctl::IntegerSetter::<KDSETMODE>::new_usize(
                    (if enable { KD_GRAPHICS } else { KD_TEXT }) as usize,
                ),
            )
        };

        if let Err(err) = result {
            log::error!(
                "Could not set KD graphics mode to {}: {}",
                if enable { "graphics" } else { "text" },
                err
            );
            return Err(err.into());
        }

        Ok(())
    }
}
