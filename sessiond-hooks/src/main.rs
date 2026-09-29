mod config;
#[path = "../../shared/logging.rs"]
mod logging;

use calloop::{generic, timer};
use clap::{Parser, ValueEnum};
use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::session_core_manager;
use hyprwire_protocols_sessiond::client::session_management::v2::session_management_v2;
use hyprwire_protocols_sessiond::client::session_management::v2::session_management_v2::{
    session_management_cgroup, session_management_session, session_manager,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::{env, fs, io, path, process, time};
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

struct Hook {
    uid: u32,
    restarts: u32,
    state: HookState,
}

enum HookState {
    Stopped,
    WaitingForEnv {
        object: session_management_session::SessionManagementSession,
    },
    WaitingForCgroup {
        envs: BTreeMap<String, String>,
        object: session_management_session::SessionManagementSession,
        cgroup_object: session_management_cgroup::SessionManagementCgroup,
    },
    Running {
        _object: session_management_session::SessionManagementSession,
        _cgroup_object: session_management_cgroup::SessionManagementCgroup,
        child: process::Child,
    },
}

struct SessiondHooks {
    config: config::Config,
    loop_handle: calloop::LoopHandle<'static, Self>,
    session_manager: Option<session_manager::SessionManager>,
    hooks: Vec<Hook>,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for SessiondHooks {
    fn event(
        &mut self,
        _object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::UserAdded { uid } => {
                log::info!(uid; "new user received");
                if !self.hooks.iter().any(|hook| hook.uid == uid)
                    && let Some(session_manager) = self.session_manager.as_ref()
                    && let Some(session) = session_manager.send_create_session::<Self>(uid)
                {
                    log::info!(uid; "creating user hook session");

                    session.send_class(session_management_v2::SessionClass::Manager);

                    self.hooks.push(Hook {
                        uid,
                        state: HookState::WaitingForEnv { object: session },
                        restarts: 0,
                    });
                }
            }
            session_core_manager::Event::Error { code, message } => {
                log::error!(code:? = code, message:% = message; "session_core_manager error");
            }
            _ => {}
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
        if let session_management_session::Event::Env { env } = event {
            let Some(hook) = self.hooks.iter_mut().find(|hook| {
                matches!(
                    &hook.state,
                    HookState::WaitingForEnv {
                        object: hook_object,
                    } if object == hook_object
                )
            }) else {
                return;
            };

            let HookState::WaitingForEnv { object } = &hook.state else {
                return;
            };

            if let Some(cgroup_object) = object.send_delegate_cgroup::<Self>() {
                let envs = env
                    .iter()
                    .filter_map(|entry| {
                        let (name, value) = entry.split_once('=')?;
                        Some((name.to_owned(), value.to_owned()))
                    })
                    .collect();

                hook.state = HookState::WaitingForCgroup {
                    envs,
                    object: object.clone(),
                    cgroup_object,
                };
            }
        }
    }
}

impl hyprwire::Dispatch<session_management_cgroup::SessionManagementCgroup> for SessiondHooks {
    fn event(
        &mut self,
        object: &session_management_cgroup::SessionManagementCgroup,
        event: <session_management_cgroup::SessionManagementCgroup as hyprwire::Object>::Event<'_>,
    ) {
        if let session_management_cgroup::Event::Path { path } = event {
            let Some(hook) = self.hooks.iter_mut().find(|hook| {
                matches!(
                    &hook.state,
                    HookState::WaitingForCgroup {
                        object: _,
                        envs,
                        cgroup_object,
                    } if object == cgroup_object
                )
            }) else {
                return;
            };

            let HookState::WaitingForCgroup {
                envs,
                object,
                cgroup_object,
            } = &hook.state
            else {
                return;
            };

            let Some(user) = uzers::get_user_by_uid(hook.uid) else {
                log::error!(uid = hook.uid; "cannot start hook for unknown uid");
                return;
            };

            let procs = path::PathBuf::from(path).join("cgroup.procs");
            let uid = rustix::thread::Uid::from_raw(hook.uid);
            let gid = rustix::thread::Gid::from_raw(user.primary_group_id());
            let groups = user
                .groups()
                .unwrap_or_default()
                .into_iter()
                .map(|group| rustix::thread::Gid::from_raw(group.gid()))
                .collect::<Vec<_>>();

            let command = self
                .config
                .on_start
                .expand_home(user.home_dir().to_string_lossy().as_ref());
            let mut cmd = process::Command::new("/bin/sh");
            unsafe {
                cmd.pre_exec(move || {
                    let pid = process::id().to_string();

                    let mut file =
                        fs::OpenOptions::new()
                            .write(true)
                            .open(&procs)
                            .map_err(|error| {
                                io::Error::new(
                                    error.kind(),
                                    format!("failed to open {}: {error}", procs.display()),
                                )
                            })?;
                    file.write_all(pid.as_bytes()).map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!("failed to write {}: {error}", procs.display()),
                        )
                    })?;

                    rustix::thread::set_thread_groups(&groups)?;
                    rustix::thread::set_thread_gid(gid)?;
                    rustix::thread::set_thread_uid(uid)?;

                    Ok(())
                })
            };
            cmd.arg("-c")
                .arg(
                    r#"
                    [ -r /etc/profile ] && . /etc/profile
                    exec "$@"
                    "#,
                )
                .arg("sessiond-hook");

            for (key, value) in envs {
                cmd.env(key, value);
            }

            cmd.arg(&command.exec);
            for arg in &command.args {
                cmd.arg(arg);
            }
            cmd.stdin(process::Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(child) => {
                    object.send_commit();

                    child
                }
                Err(e) => {
                    log::error!(
                        exec:? = command.exec,
                        uid = hook.uid,
                        error:% = e;
                        "failed to spawn hook"
                    );
                    return;
                }
            };

            let Some(pid) = i32::try_from(child.id())
                .ok()
                .and_then(rustix::process::Pid::from_raw)
            else {
                log::error!(pid = child.id(); "cannot supervise hook with invalid pid");
                let _ = child.kill();
                let _ = child.wait();
                return;
            };
            let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
            {
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

            let fd_wrapper = unsafe { generic::FdWrapper::new(pidfd) };
            let source = generic::Generic::new(
                fd_wrapper,
                calloop::Interest {
                    readable: true,
                    writable: false,
                },
                calloop::Mode::Level,
            );

            let uid = hook.uid;
            if let Err(err) = self.loop_handle.insert_source(source, move |_, _, state| {
                let Some(hook) = state.hooks.iter_mut().find(|hook| hook.uid == uid) else {
                    return Ok(calloop::PostAction::Remove);
                };
                let HookState::Running { child, .. } = &mut hook.state else {
                    return Ok(calloop::PostAction::Reregister);
                };
                let Ok(Some(status)) = child.try_wait() else {
                    return Ok(calloop::PostAction::Reregister);
                };

                match (state.config.restart.on, status.success()) {
                    (config::RestartPolicy::Never, _) | (config::RestartPolicy::Failure, true) => {
                        hook.state = HookState::Stopped;
                    }
                    (config::RestartPolicy::Always, _)
                    | (config::RestartPolicy::Failure, false) => {
                        let restart_delay = state.config.restart.delay;
                        let restart =
                            timer::Timer::from_duration(time::Duration::from_secs(restart_delay));
                        if let Err(error) =
                            state
                                .loop_handle
                                .insert_source(restart, move |_, (), state| {
                                    if let Some(hook) =
                                        state.hooks.iter_mut().find(|hook| hook.uid == uid)
                                    {
                                        hook.restarts += 1;

                                        log::info!(
                                            uid,
                                            delay_seconds = restart_delay,
                                            failures = hook.restarts;
                                            "restarting hook"
                                        );

                                        if let Some(session_manager) =
                                            state.session_manager.as_ref()
                                            && let Some(session) =
                                                session_manager.send_create_session::<Self>(uid)
                                        {
                                            session.send_class(
                                                session_management_v2::SessionClass::Manager,
                                            );
                                            hook.state =
                                                HookState::WaitingForEnv { object: session };
                                        }
                                    }

                                    timer::TimeoutAction::Drop
                                })
                        {
                            log::error!(
                                uid,
                                error:% = error;
                                "failed to schedule restart"
                            );
                        }
                    }
                }

                Ok(calloop::PostAction::Remove)
            }) {
                log::error!(
                    uid = hook.uid,
                    error:% = err;
                    "failed to supervise hook"
                );
                let _ = child.kill();
                let _ = child.wait();
                return;
            }

            hook.state = HookState::Running {
                _object: object.clone(),
                _cgroup_object: cgroup_object.clone(),
                child,
            }
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

    let mut event_loop = calloop::EventLoop::try_new()?;

    let sessiond_runtime_dir: path::PathBuf = env::var_os("SESSIOND_RUNTIME_DIR")
        .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
    let mut socket = client::Client::connect(sessiond_runtime_dir.join("sessiond.sock"))?;
    let eq = socket.new_event_queue();

    let mut sessiond_hooks = SessiondHooks {
        hooks: Vec::new(),
        loop_handle: event_loop.handle(),
        session_manager: None,
        config,
    };

    socket.add_implementation::<session_management_v2::SessionManagementV2>();
    socket.add_implementation::<session_core_v1::SessionCoreV1>();

    eq.wait_for_handshake(&mut sessiond_hooks)?;

    sessiond_hooks.session_manager =
        Some(socket.bind::<session_manager::SessionManager, _>(&eq, &mut sessiond_hooks, 1)?);
    log::info!("session manager connected");
    let _session_core_manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut sessiond_hooks, 1)?;

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
