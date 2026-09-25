mod config;
mod ecs;
#[path = "../../shared/logging.rs"]
mod logging;
mod session;

use anyhow::Context;
use bevy_ecs::message;
use bevy_ecs::prelude::*;
use calloop::{generic, signals};
use clap::{Parser, ValueEnum};
use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use posix_acl as acl;
use session_core_v1::session_core_manager;
use std::{env, path, process, sync, time};

struct SessiondUaccess {
    reconcile: Schedule,
    world: World,
}

fn revoke_stale_grants(
    mut commands: Commands,
    grants: Query<(Entity, &ecs::DeviceEntity, &ecs::AccessMode, &ecs::Uid), With<ecs::Grant>>,
    devices: Query<(&ecs::Syspath, &ecs::Devnode), With<ecs::Device>>,
    sessions: Query<(&ecs::Uid, &ecs::Done, &ecs::IsLocal, &ecs::ActiveState), With<ecs::Session>>,
) {
    for (grant_entity, device_entity, access_mode, grant_uid) in &grants {
        if sessions.iter().any(|(uid, done, is_local, active)| {
            uid.0 == grant_uid.0
                && done.0
                && is_local.0
                && active.0 == session_core_v1::ActiveState::Active
        }) {
            continue;
        }

        let Ok((syspath, devnode)) = devices.get(device_entity.0) else {
            commands.entity(grant_entity).despawn();
            continue;
        };

        let mut acl = match acl::PosixACL::read_acl(&devnode.0) {
            Ok(acl) => acl,
            Err(e) => {
                log::error!(
                    syspath:% = syspath.0.display(),
                    devnode:% = devnode.0.display(),
                    uid = grant_uid.0,
                    access = access_mode.0.as_str(),
                    error = e.to_string().as_str();
                    "failed to read device acl"
                );
                continue;
            }
        };
        acl.remove(acl::Qualifier::User(grant_uid.0));
        if let Err(e) = acl.write_acl(&devnode.0) {
            log::error!(
                syspath:% = syspath.0.display(),
                devnode:% = devnode.0.display(),
                uid = grant_uid.0,
                access = access_mode.0.as_str(),
                error = e.to_string().as_str();
                "failed to write device acl"
            );
            continue;
        }

        commands.entity(grant_entity).despawn();

        log::info!(
            syspath:% = syspath.0.display(),
            devnode:% = devnode.0.display(),
            uid = grant_uid.0,
            access = access_mode.0.as_str();
            "removed device grant"
        );
    }
}

fn apply_missing_grants(
    mut commands: Commands,
    devices: Query<(Entity, &ecs::Syspath, &ecs::Devnode, &ecs::AccessMode), With<ecs::Device>>,
    sessions: Query<(&ecs::Uid, &ecs::Done, &ecs::IsLocal, &ecs::ActiveState), With<ecs::Session>>,
    grants: Query<(&ecs::DeviceEntity, &ecs::Uid), With<ecs::Grant>>,
) {
    for (uid, done, is_local, active) in &sessions {
        if !done.0 || !is_local.0 || active.0 != session_core_v1::ActiveState::Active {
            continue;
        }

        for (device_entity, syspath, devnode, access_mode) in &devices {
            let grant_exists = grants.iter().any(|(grant_device_entity, grant_uid)| {
                grant_device_entity.0 == device_entity && grant_uid.0 == uid.0
            });
            if grant_exists {
                continue;
            }

            let mut acl = match acl::PosixACL::read_acl(&devnode.0) {
                Ok(acl) => acl,
                Err(e) => {
                    log::error!(
                        syspath:% = syspath.0.display(),
                        devnode:% = devnode.0.display(),
                        uid = uid.0,
                        access = access_mode.0.as_str(),
                        error = e.to_string().as_str();
                        "failed to read device acl"
                    );
                    continue;
                }
            };

            let acl_rule = match access_mode.0 {
                config::AccessMode::Read => acl::ACL_READ,
                config::AccessMode::Write => acl::ACL_WRITE,
                config::AccessMode::ReadWrite => acl::ACL_READ | acl::ACL_WRITE,
            };
            acl.set(acl::Qualifier::User(uid.0), acl_rule);
            if let Err(e) = acl.write_acl(&devnode.0) {
                log::error!(
                    syspath:% = syspath.0.display(),
                    devnode:% = devnode.0.display(),
                    uid = uid.0,
                    access = access_mode.0.as_str(),
                    error = e.to_string().as_str();
                    "failed to write device acl"
                );
                continue;
            }

            commands.spawn((
                ecs::Grant,
                ecs::DeviceEntity(device_entity),
                ecs::AccessMode(access_mode.0),
                ecs::Uid(uid.0),
            ));

            log::info!(
                syspath:% = syspath.0.display(),
                devnode:% = devnode.0.display(),
                uid = uid.0,
                access = access_mode.0.as_str();
                "inserted device grant"
            );
        }
    }
}

fn process_device_removals(
    mut message: MessageReader<ecs::DeviceRemoved>,
    devices: Query<(Entity, &ecs::Syspath), With<ecs::Device>>,
    grants: Query<(Entity, &ecs::DeviceEntity), With<ecs::Grant>>,
    mut commands: Commands,
) {
    for removed in message.read() {
        let Some((device_entity, _)) = devices
            .iter()
            .find(|(_, syspath)| syspath.0.as_ref() == removed.syspath.as_ref())
        else {
            continue;
        };

        for (grant_entity, grant_device) in &grants {
            if grant_device.0 == device_entity {
                commands.entity(grant_entity).despawn();
            }
        }

        commands.entity(device_entity).despawn();
    }
}

#[allow(clippy::needless_pass_by_value)]
fn reconcile_devices(
    mut message: MessageReader<ecs::DeviceAdded>,
    mut commands: Commands,
    devices: Query<
        (
            Entity,
            &ecs::Syspath,
            Option<&ecs::Devnode>,
            Option<&ecs::AccessMode>,
        ),
        With<ecs::Device>,
    >,
    grants: Query<(Entity, &ecs::DeviceEntity), With<ecs::Grant>>,
    config: NonSend<config::Config>,
) {
    for syspath in message.read().map(|added| sync::Arc::clone(&added.syspath)) {
        let device = match udev::Device::from_syspath(&syspath) {
            Ok(device) => config::LuaDevice::new(device),
            Err(error) => {
                log::debug!(
                    syspath:% = syspath.display(),
                    error = error.to_string().as_str();
                    "failed to resolve device during reconciliation"
                );
                continue;
            }
        };

        let started = time::Instant::now();
        let Some(devnode) = device.device.devnode() else {
            log::debug!(syspath:% = syspath.display(); "skipping device without devnode");
            continue;
        };
        let subsystem = device
            .device
            .subsystem()
            .map(|subsystem| subsystem.to_string_lossy().into_owned());
        let sysname = device.device.sysname().to_string_lossy().into_owned();

        for rule in config.rules.borrow().iter() {
            let result = {
                let match_fn = config
                    .lua
                    .registry_value::<mlua::Function>(&rule.match_fn)
                    .map_err(|error| anyhow::anyhow!("{error}"));
                match_fn.and_then(|match_fn| {
                    match_fn
                        .call::<bool>(device.clone())
                        .map(|matched| (matched, rule.access))
                        .map_err(|error| anyhow::anyhow!("{error}"))
                })
            };

            let (matched, access) = match result {
                Ok(result) => result,
                Err(error) => {
                    log::error!(
                        syspath:% = syspath.display(),
                        error = error.to_string().as_str();
                        "failed to evaluate device match rule"
                    );
                    continue;
                }
            };

            if matched {
                log::debug!(
                    syspath:% = syspath.display(),
                    devnode:% = devnode.display(),
                    subsystem = subsystem.as_deref(),
                    sysname = sysname.as_str();
                    "device matched rule"
                );

                if let Some((device_entity, _, existing_devnode, existing_access_mode)) = devices
                    .iter()
                    .find(|(_, device_syspath, _, _)| device_syspath.0 == syspath)
                {
                    let device_changed = existing_devnode
                        .is_none_or(|existing_devnode| existing_devnode.0 != devnode)
                        || existing_access_mode.is_none_or(|access_mode| access_mode.0 != access);

                    if device_changed {
                        for (grant_entity, grant_device) in &grants {
                            if grant_device.0 == device_entity {
                                commands.entity(grant_entity).despawn();
                            }
                        }
                        commands.entity(device_entity).despawn();
                        commands.spawn((
                            ecs::Device,
                            ecs::Syspath(sync::Arc::clone(&syspath)),
                            ecs::Devnode(devnode.to_owned()),
                            ecs::AccessMode(access),
                        ));
                    }
                } else {
                    commands.spawn((
                        ecs::Device,
                        ecs::Syspath(sync::Arc::clone(&syspath)),
                        ecs::Devnode(devnode.to_owned()),
                        ecs::AccessMode(access),
                    ));
                    log::info!(
                        syspath:% = syspath.display(),
                        devnode:% = devnode.display();
                        "inserted managed device"
                    );
                }

                break;
            }
        }

        log::debug!(
            syspath:% = syspath.display(),
            devnode:% = devnode.display(),
            duration:? = started.elapsed();
            "reconciled device"
        );
    }
}

#[derive(Parser)]
struct Args {
    #[arg(long)]
    rules_dirs: Vec<path::PathBuf>,
    #[arg(long, value_enum, default_value_t = LogTarget::Stderr)]
    log_target: LogTarget,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogTarget {
    Stderr,
    Syslog,
}

fn main() -> process::ExitCode {
    let startup_started = time::Instant::now();
    let args = Args::parse();
    if let Err(error) = init_logging(args.log_target) {
        eprintln!("failed to initialize logging: {error:#}");
        return process::ExitCode::FAILURE;
    }

    let status = match run(args, startup_started) {
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
                .filter(Some("sessiond_uaccess"), log_level)
                .try_init()?;
        }
        LogTarget::Syslog => {
            logging::init_syslog(syslog::Facility::LOG_DAEMON, log_level, "sessiond-uaccess")?;
        }
    }
    log::info!(target:? = log_target, level:? = log_level; "logging initialized");
    Ok(())
}

fn run(args: Args, startup_started: time::Instant) -> anyhow::Result<()> {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            message::message_update_system,
            reconcile_devices,
            process_device_removals,
            revoke_stale_grants,
            apply_missing_grants,
        )
            .chain(),
    );

    let mut world = World::new();
    message::MessageRegistry::register_message::<ecs::DeviceRemoved>(&mut world);
    message::MessageRegistry::register_message::<ecs::DeviceAdded>(&mut world);
    world.insert_non_send(
        config::Config::load(&args.rules_dirs).context("failed to load Lua rules")?,
    );

    let mut sessiond_uaccess = SessiondUaccess {
        world,
        reconcile: schedule,
    };

    log::info!(
        rules_dirs = args.rules_dirs.len(),
        rules = sessiond_uaccess.world.non_send::<config::Config>().rules.borrow().len();
        "loaded access rules"
    );

    let mut event_loop = calloop::EventLoop::try_new()?;

    let loop_signal = event_loop.get_signal();

    let signals = signals::Signals::new(&[signals::Signal::SIGTERM, signals::Signal::SIGINT])?;
    event_loop
        .handle()
        .insert_source(signals, move |event, (), state: &mut SessiondUaccess| {
            log::info!(signal:? = event.signal(); "terminating");

            let sessions: Vec<_> = {
                let mut sessions = state
                    .world
                    .query_filtered::<(Entity, &ecs::Uid), With<ecs::Session>>();
                sessions
                    .iter(&state.world)
                    .map(|(entity, uid)| (entity, uid.0))
                    .collect()
            };

            for (session_entity, _) in sessions {
                state.world.despawn(session_entity);
            }

            loop_signal.stop();
            loop_signal.wakeup();
        })?;

    let sessiond_runtime_dir: path::PathBuf = env::var_os("SESSIOND_RUNTIME_DIR")
        .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
    let mut socket = client::Client::connect(sessiond_runtime_dir.join("sessiond.sock"))
        .context("failed to connect to /run/sessiond/sessiond.sock")?;
    let eq = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1>();

    eq.wait_for_handshake(&mut sessiond_uaccess)
        .context("sessiond protocol handshake failed")?;

    let _session_core_manager = socket
        .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut sessiond_uaccess, 1)
        .context("failed to bind sessiond manager")?;

    log::info!(socket = "/run/sessiond/sessiond.sock"; "connected to sessiond");

    eq.roundtrip(&mut sessiond_uaccess)?;

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
            if let Err(error) = eq.dispatch_events(state, false) {
                let error = error.to_string();
                log::error!(error = error.as_str(); "failed to dispatch sessiond events");
            }
            Ok(calloop::PostAction::Continue)
        })?;

    let monitor = udev::MonitorBuilder::new()?.listen()?;

    let mut enumerator = udev::Enumerator::new()?;
    for device in enumerator.scan_devices()? {
        log::trace!(
            syspath = device.syspath().to_string_lossy().as_ref();
            "reconciling existing device"
        );
        sessiond_uaccess.world.write_message(ecs::DeviceAdded {
            syspath: device.syspath().into(),
        });
    }

    event_loop
        .handle()
        .insert_source(
            generic::Generic::new(monitor, calloop::Interest::READ, calloop::Mode::Level),
            |_, monitor, state: &mut SessiondUaccess| {
                for event in monitor.iter() {
                    match event.event_type() {
                        udev::EventType::Add | udev::EventType::Change => {
                            state.world.write_message(ecs::DeviceAdded {
                                syspath: event.device().syspath().into(),
                            });
                        }
                        udev::EventType::Remove => {
                            state.world.write_message(ecs::DeviceRemoved {
                                syspath: event.device().syspath().into(),
                            });
                        }
                        _ => {}
                    }
                }

                Ok(calloop::PostAction::Continue)
            },
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let wakeup_started = time::Instant::now();
    event_loop.get_signal().wakeup();
    log::debug!(
        duration:? = startup_started.elapsed();
        "queued initial device reconciliation wakeup"
    );

    let mut first_schedule = true;
    event_loop.run(None, &mut sessiond_uaccess, move |state| {
        let reconciliation_started = time::Instant::now();
        state.reconcile.run(&mut state.world);
        let reconciliation_duration = reconciliation_started.elapsed();

        if first_schedule {
            first_schedule = false;
            log::debug!(
                startup_duration:? = startup_started.elapsed(),
                wakeup_duration:? = wakeup_started.elapsed(),
                reconciliation_duration:? = reconciliation_duration;
                "completed startup reconciliation"
            );
        }
    })?;

    Ok(())
}
