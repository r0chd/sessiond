use hyprwire_protocols_sessiond::client::session_power::v1::session_power_v1;

use anyhow::Context;
use hyprwire::client;
use session_power_v1::session_power_manager;

const SOCKET_PATH: &str = "/run/sessiond/power.sock";

#[derive(Clone, Copy)]
pub enum Action {
    Reboot,
    Poweroff,
    Suspend,
    Hibernate,
    HybridSleep,
}

#[derive(Default)]
struct State {
    done: bool,
    error: Option<String>,
}

impl hyprwire::Dispatch<session_power_manager::SessionPowerManager> for State {
    fn event(
        &mut self,
        _object: &session_power_manager::SessionPowerManager,
        event: <session_power_manager::SessionPowerManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_power_manager::Event::Done => self.done = true,
            session_power_manager::Event::RebootFailed => {
                self.error = Some("reboot request failed".to_string());
                self.done = true;
            }
            session_power_manager::Event::PoweroffFailed => {
                self.error = Some("poweroff request failed".to_string());
                self.done = true;
            }
            session_power_manager::Event::SuspendFailed => {
                self.error = Some("suspend request failed".to_string());
                self.done = true;
            }
            session_power_manager::Event::HibernateFailed => {
                self.error = Some("hibernate request failed".to_string());
                self.done = true;
            }
            session_power_manager::Event::HybridSleepFailed => {
                self.error = Some("hybrid sleep request failed".to_string());
                self.done = true;
            }
            _ => {}
        }
    }
}

/// Requests `action` from the sessiond power service and waits for its result.
///
/// # Errors
///
/// Returns an error if the service cannot be reached, the Hyprwire exchange
/// fails, or the service rejects or cannot execute the requested action.
pub fn run(action: Action) -> anyhow::Result<()> {
    let mut state = State::default();
    let mut socket = client::Client::connect(SOCKET_PATH)
        .with_context(|| format!("failed to connect to {SOCKET_PATH}"))?;
    let queue = socket.new_event_queue();

    socket.add_implementation::<session_power_v1::SessionPowerV1>();
    queue.wait_for_handshake(&mut state)?;

    let manager =
        socket.bind::<session_power_manager::SessionPowerManager, _>(&queue, &mut state, 1)?;

    match action {
        Action::Reboot => manager.send_reboot(),
        Action::Poweroff => manager.send_poweroff(),
        Action::Suspend => manager.send_suspend(),
        Action::Hibernate => manager.send_hibernate(),
        Action::HybridSleep => manager.send_hybrid_sleep(),
    }

    while !state.done {
        queue.dispatch_events(&mut state, true)?;
    }

    if let Some(message) = state.error {
        anyhow::bail!(message);
    }

    Ok(())
}
