mod config;
#[path = "../../shared/logging.rs"]
mod logging;

use anyhow::Context;
use calloop::{generic, timer};
use clap::{Parser, ValueEnum};
use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_manager, session_core_user,
};
use hyprwire_protocols_sessiond::client::session_management::v2::session_management_v2;
use hyprwire_protocols_sessiond::client::session_management::v2::session_management_v2::{
    session_management_session, session_manager,
};
use std::collections::BTreeMap;
use std::os::unix::process::CommandExt;
use std::{env, fs, io, ops, path, process, time};
use uzers::os::unix::UserExt;

#[derive(Parser)]
#[command(version, about = "Launch and supervise a per-user session hook")]
struct Cli {
    #[arg(
        long,
        value_name = "PATH",
        help = "Path to the TOML configuration file"
    )]
    config: Option<path::PathBuf>,
    #[arg(long, value_enum, default_value_t = LogTarget::Stderr)]
    log_target: LogTarget,
}

#[derive(Clone, Copy, ValueEnum)]
enum LogTarget {
    Stderr,
    Syslog,
}

struct PendingManagedSession {
    id: u32,
    uid: u32,
    object: session_management_session::SessionManagementSession,
    envs: BTreeMap<String, String>,
}

struct ManagedSession {
    id: u32,
    uid: u32,
    _object: session_management_session::SessionManagementSession,
    envs: BTreeMap<String, String>,
    hook: Option<process::Child>,
}

struct User {
    uid: u32,
    state: session_core_v1::UserState,
    state_generation: u64,
    object: session_core_user::SessionCoreUser,
}

impl ManagedSession {
    fn run_hook(
        &mut self,
        config: &config::Config,
        loop_handle: calloop::LoopHandle<SessiondHooks>,
    ) {
        let Some(user) = uzers::get_user_by_uid(self.uid) else {
            log::error!(uid = self.uid; "cannot start hook for unknown uid");
            return;
        };
        let command = config
            .on_start
            .expand_home(user.home_dir().to_string_lossy().as_ref());
        let mut cmd = process::Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(
                r#"
                [ -r /etc/profile ] && . /etc/profile

                exec env "$@"
                "#,
            )
            .arg("sessiond-hook");
        for (key, value) in &self.envs {
            cmd.arg(format!("{key}={value}"));
        }
        cmd.arg(&command.exec);
        for arg in &command.args {
            cmd.arg(arg);
        }
        cmd.uid(self.uid).gid(user.primary_group_id());
        for (key, value) in &self.envs {
            cmd.env(key, value);
        }

        log::info!(
            exec:? = command.exec,
            args:? = command.args,
            session_id = self.id,
            uid = self.uid,
            env_count = self.envs.len();
            "spawning hook"
        );
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                log::error!(
                    exec:? = command.exec,
                    uid = self.uid,
                    error:% = err;
                    "failed to spawn hook"
                );
                return;
            }
        };
        log::info!(exec:? = command.exec, pid = child.id(),
            session_id = self.id, uid = self.uid; "spawned hook");

        let Some(pid) = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        else {
            log::error!(pid = child.id(); "cannot supervise hook with invalid pid");
            let _ = child.kill();
            let _ = child.wait();
            return;
        };
        let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(err) => {
                log::error!(
                   pid = child.id(),
                   error:% = err;
                   "cannot open pidfd for hook"
                );
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        };

        self.hook = Some(child);

        let fd_wrapper = unsafe { generic::FdWrapper::new(pidfd) };
        let source = generic::Generic::new(
            fd_wrapper,
            calloop::Interest {
                readable: true,
                writable: false,
            },
            calloop::Mode::Level,
        );

        let session_id = self.id;
        if let Err(err) = loop_handle.insert_source(source, move |_, _, state| {
            let Some(session) = state
                .managed_sessions
                .iter_mut()
                .find(|session| session.id == session_id)
            else {
                log::debug!(
                    session_id;
                    "discarding child-exit event for removed session"
                );
                return Ok(calloop::PostAction::Remove);
            };

            if let Some(child) = &mut session.hook
                && let Some(status) = child.try_wait()?
            {
                match (state.config.restart.on, status.success()) {
                    (config::RestartPolicy::Never, _) | (config::RestartPolicy::Failure, true) => {}
                    (config::RestartPolicy::Always, _)
                    | (config::RestartPolicy::Failure, false) => {
                        let restart_delay = state.config.restart.delay;
                        let restart =
                            timer::Timer::from_duration(time::Duration::from_secs(restart_delay));
                        if let Err(error) =
                            state
                                .loop_handle
                                .insert_source(restart, move |_, (), state| {
                                    if let Some(session) = state
                                        .managed_sessions
                                        .iter_mut()
                                        .find(|session| session.id == session_id)
                                    {
                                        log::info!(session_id = session.id,
                                            uid = session.uid,
                                            delay_seconds = restart_delay;
                                            "restarting hook"
                                        );
                                        session.run_hook(&state.config, state.loop_handle.clone());
                                    }

                                    timer::TimeoutAction::Drop
                                })
                        {
                            log::error!(
                                session_id,
                                error:% = error;
                                "failed to schedule restart"
                            );
                        }
                    }
                }
                log::info!(
                    session_id,
                    status:% = status;
                    "hook exited"
                );

                session.hook = None;

                return Ok(calloop::PostAction::Remove);
            }

            Ok(calloop::PostAction::Continue)
        }) {
            log::error!(
                session_id,
                error:% = err;
                "failed to supervise hook"
            );
            if let Some(child) = &mut self.hook {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.hook = None;
        }
    }
}

fn normal_uid_range(login_defs: &str) -> anyhow::Result<ops::RangeInclusive<u32>> {
    let mut uid_min = None;
    let mut uid_max = None;

    for line in login_defs.lines() {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let target = match name {
            "UID_MIN" => &mut uid_min,
            "UID_MAX" => &mut uid_max,
            _ => continue,
        };
        let value = fields
            .next()
            .with_context(|| format!("{name} has no value in /etc/login.defs"))?;
        *target = Some(
            value
                .parse::<u32>()
                .with_context(|| format!("invalid {name} value in /etc/login.defs"))?,
        );
    }

    let uid_min = uid_min.context("UID_MIN is missing from /etc/login.defs")?;
    let uid_max = uid_max.context("UID_MAX is missing from /etc/login.defs")?;
    anyhow::ensure!(
        uid_min <= uid_max,
        "UID_MIN exceeds UID_MAX in /etc/login.defs"
    );

    Ok(uid_min..=uid_max)
}

fn is_hook_user(uid: u32, normal_uids: &ops::RangeInclusive<u32>) -> bool {
    uid == 0 || normal_uids.contains(&uid)
}

struct SessiondHooks {
    users: Vec<User>,
    pending_managed_sessions: Vec<PendingManagedSession>,
    managed_sessions: Vec<ManagedSession>,
    config: config::Config,
    normal_uids: ops::RangeInclusive<u32>,
    session_manager: Option<session_manager::SessionManager>,
    loop_handle: calloop::LoopHandle<'static, Self>,
}

impl SessiondHooks {
    fn create_managed_session(&mut self, uid: u32) {
        if self
            .managed_sessions
            .iter()
            .any(|session| session.uid == uid)
            || self
                .pending_managed_sessions
                .iter()
                .any(|session| session.uid == uid)
        {
            log::info!(uid; "uid already has a managed hook session");
            return;
        }

        let Some(session_manager) = self.session_manager.as_ref() else {
            log::error!(uid; "cannot create managed session: manager is unavailable");
            return;
        };

        let Some(session_object) = session_manager.send_create_session::<Self>(uid) else {
            log::error!(uid; "failed to create managed session: protocol returned no object");
            return;
        };

        let pending_session = PendingManagedSession {
            id: 0,
            uid,
            object: session_object,
            envs: BTreeMap::new(),
        };

        pending_session
            .object
            .send_class(session_management_v2::SessionClass::Manager);
        pending_session.object.send_done();

        self.pending_managed_sessions.push(pending_session);
    }

    fn finish_teardown(&mut self, uid: u32) {
        if let Some(index) = self
            .managed_sessions
            .iter()
            .position(|session| session.uid == uid)
        {
            let mut session = self.managed_sessions.swap_remove(index);
            if let Some(mut child) = session.hook.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        } else if let Some(index) = self
            .pending_managed_sessions
            .iter()
            .position(|session| session.uid == uid)
        {
            drop(self.pending_managed_sessions.swap_remove(index));
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for SessiondHooks {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::UserAdded { uid } => {
                if !is_hook_user(uid, &self.normal_uids) {
                    log::debug!(uid; "ignoring non-login user");
                    return;
                }

                if self.users.iter().any(|user| user.uid == uid) {
                    log::info!(uid; "uid is already being observed");
                    return;
                }

                let Some(user_object) = object.send_get_user::<Self>(uid) else {
                    log::error!(uid; "failed to create user observer: protocol returned no object");
                    return;
                };

                self.users.push(User {
                    uid,
                    state: session_core_v1::UserState::Online,
                    state_generation: 0,
                    object: user_object,
                });

                self.create_managed_session(uid);
            }
            session_core_manager::Event::Error { code, message } => {
                log::error!(code:? = code, message:% = message; "session_core_manager error");
            }
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for SessiondHooks {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        if let session_core_user::Event::State { state } = event {
            let Some(user) = self.users.iter_mut().find(|user| user.object == *object) else {
                return;
            };

            user.state = state;
            user.state_generation = user.state_generation.wrapping_add(1);
            let uid = user.uid;
            let state_generation = user.state_generation;

            if state == session_core_v1::UserState::Offline {
                if let Err(error) = self.loop_handle.insert_source(
                    timer::Timer::from_duration(time::Duration::from_secs(5)),
                    {
                        let object = object.clone();
                        move |_, (), state| {
                            let Some(user) = state.users.iter().find(|user| user.object == object)
                            else {
                                return timer::TimeoutAction::Drop;
                            };

                            if user.state != session_core_v1::UserState::Offline
                                || user.state_generation != state_generation
                            {
                                return timer::TimeoutAction::Drop;
                            }

                            state.finish_teardown(uid);
                            timer::TimeoutAction::Drop
                        }
                    },
                ) {
                    log::error!(uid, error:% = error; "failed to schedule offline teardown");
                }
            } else if matches!(
                state,
                session_core_v1::UserState::Lingering
                    | session_core_v1::UserState::Online
                    | session_core_v1::UserState::Active
            ) {
                self.create_managed_session(uid);
            }
        }
    }
}

impl hyprwire::Dispatch<session_management_session::SessionManagementSession> for SessiondHooks {
    fn event(
        &mut self,
        object: &session_management_session::SessionManagementSession,
        event: <session_management_session::SessionManagementSession as hyprwire::Object>::Event<
            '_,
        >,
    ) {
        match event {
            session_management_session::Event::Env { name, value } => {
                let Some(session) = self
                    .pending_managed_sessions
                    .iter_mut()
                    .find(|session| session.object == *object)
                else {
                    return;
                };

                log::debug!(name:? = name, session_id = session.id, uid = session.uid;
                    "received environment variable");
                session.envs.insert(name, value);
            }
            session_management_session::Event::SessionId { session_id } => {
                let Some(session) = self
                    .pending_managed_sessions
                    .iter_mut()
                    .find(|session| session.object == *object)
                else {
                    return;
                };

                session.id = session_id;
                log::info!(session_id, uid = session.uid; "assigned management session object");
            }
            session_management_session::Event::Done => {
                let Some(session) = self
                    .pending_managed_sessions
                    .iter()
                    .position(|session| session.object == *object)
                else {
                    return;
                };
                let session = self.pending_managed_sessions.swap_remove(session);

                let Some(user) = uzers::get_user_by_uid(session.uid) else {
                    log::error!(uid = session.uid; "cannot start hook for unknown uid");
                    return;
                };

                let mut managed_session = ManagedSession {
                    id: session.id,
                    uid: session.uid,
                    _object: session.object,
                    envs: session.envs,
                    hook: None,
                };
                managed_session.envs.extend(BTreeMap::from([
                    (
                        "HOME".to_string(),
                        user.home_dir().to_string_lossy().into_owned(),
                    ),
                    (
                        "USER".to_string(),
                        user.name().to_string_lossy().into_owned(),
                    ),
                    (
                        "LOGNAME".to_string(),
                        user.name().to_string_lossy().into_owned(),
                    ),
                    (
                        "SHELL".to_string(),
                        user.shell().to_string_lossy().into_owned(),
                    ),
                ]));
                managed_session.run_hook(&self.config, self.loop_handle.clone());

                self.managed_sessions.push(managed_session);
            }
            session_management_session::Event::Error { code, message } => {
                if let Some(index) = self
                    .pending_managed_sessions
                    .iter()
                    .position(|session| session.object == *object)
                {
                    let session = self.pending_managed_sessions.swap_remove(index);
                    log::error!(
                        uid = session.uid,
                        session_id = session.id,
                        code:? = code,
                        message:% = message;
                        "managed session creation failed"
                    );
                }
            }
            _ => {}
        }
    }
}

fn main() -> process::ExitCode {
    let cli = Cli::parse();
    if let Err(error) = init_logging(cli.log_target) {
        eprintln!("failed to initialize logging: {error:#}");
        return process::ExitCode::FAILURE;
    }

    let status = match run(cli) {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(error) => {
            log::error!("application failed: {error:#}");
            process::ExitCode::FAILURE
        }
    };
    log::logger().flush();
    status
}

fn init_logging(log_target: LogTarget) -> anyhow::Result<()> {
    let log_level = match env::var("LOG_LEVEL") {
        Ok(log) if log == "trace" => log::LevelFilter::Trace,
        Ok(log) if log == "debug" => log::LevelFilter::Debug,
        Ok(log) if log == "warn" => log::LevelFilter::Warn,
        Ok(log) if log == "error" => log::LevelFilter::Error,
        Ok(log) if log == "off" => log::LevelFilter::Off,
        _ => log::LevelFilter::Info,
    };

    match log_target {
        LogTarget::Stderr => {
            env_logger::Builder::new()
                .filter(Some("sessiond_hooks"), log_level)
                .try_init()?;
        }
        LogTarget::Syslog => {
            logging::init_syslog(syslog::Facility::LOG_DAEMON, log_level, "sessiond-hooks")?;
        }
    }
    Ok(())
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let config = config::Config::load(cli.config)?;
    let normal_uids = normal_uid_range(
        &fs::read_to_string("/etc/login.defs").context("reading /etc/login.defs")?,
    )?;

    let mut event_loop = calloop::EventLoop::try_new()?;

    let sessiond_runtime_dir: path::PathBuf = env::var_os("SESSIOND_RUNTIME_DIR")
        .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
    let mut socket = client::Client::connect(sessiond_runtime_dir.join("sessiond.sock"))?;
    let eq = socket.new_event_queue();

    let mut sessiond_hooks = SessiondHooks {
        loop_handle: event_loop.handle(),
        users: Vec::new(),
        managed_sessions: Vec::new(),
        pending_managed_sessions: Vec::new(),
        normal_uids,
        session_manager: None,
        config,
    };

    socket.add_implementation::<session_management_v2::SessionManagementV2>();
    socket.add_implementation::<session_core_v1::SessionCoreV1>();

    eq.wait_for_handshake(&mut sessiond_hooks)?;

    sessiond_hooks.session_manager =
        Some(socket.bind::<session_manager::SessionManager, _>(&eq, &mut sessiond_hooks, 1)?);
    let _session_core_manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut sessiond_hooks, 2)?;

    eq.roundtrip(&mut sessiond_hooks)?;

    let fd_wrapper =
        unsafe { generic::FdWrapper::new(socket.extract_loop_fd().try_clone_to_owned()?) };
    let source = generic::Generic::new(
        fd_wrapper,
        calloop::Interest {
            readable: true,
            writable: false,
        },
        calloop::Mode::Level,
    );
    event_loop
        .handle()
        .insert_source(source, move |_, _, state| {
            eq.dispatch_events(state, false).map_err(io::Error::other)?;
            Ok(calloop::PostAction::Continue)
        })?;

    event_loop.run(None, &mut sessiond_hooks, |_| {})?;

    Ok(())
}

hyprwire::delegate_noop!(SessiondHooks: session_manager::SessionManager);
