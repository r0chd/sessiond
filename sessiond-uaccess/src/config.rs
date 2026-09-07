use std::{cell, fs, path, rc};

pub struct Config {
    pub lua: mlua::Lua,
    pub rules: rc::Rc<cell::RefCell<Vec<LuaRule>>>,
}

impl Config {
    pub fn load(rule_dirs: &[path::PathBuf]) -> anyhow::Result<Self> {
        let lua = mlua::Lua::new_with(
            mlua::StdLib::TABLE | mlua::StdLib::STRING | mlua::StdLib::MATH | mlua::StdLib::UTF8,
            mlua::LuaOptions::default(),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        lua.set_memory_limit(512 * 1024)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let globals = lua.globals();
        globals
            .set("api_version", 1)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let rules = rc::Rc::new(cell::RefCell::new(Vec::new()));

        let device_fn = lua
            .create_function({
                let rules = rc::Rc::clone(&rules);
                move |lua, rule: mlua::Table| {
                    let access = match rule.get::<String>("access")?.as_str() {
                        "r" => AccessMode::Read,
                        "w" => AccessMode::Write,
                        "rw" => AccessMode::ReadWrite,
                        other => {
                            return Err(mlua::Error::RuntimeError(format!(
                                "invalid access mode: {other}"
                            )));
                        }
                    };

                    let match_fn =
                        lua.create_registry_value(rule.get::<mlua::Function>("match")?)?;

                    rules.borrow_mut().push(LuaRule { access, match_fn });

                    Ok(())
                }
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        lua.globals()
            .set("device", device_fn)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        for dir in rule_dirs.iter().filter(|dir| dir.exists()) {
            let mut rule_paths = fs::read_dir(dir)?
                .map(|rule| rule.map(|rule| rule.path()))
                .collect::<Result<Vec<_>, _>>()?;
            rule_paths.retain(|path| path.extension().is_some_and(|extension| extension == "lua"));
            rule_paths.sort();

            for path in rule_paths {
                let source = fs::read_to_string(&path)?;

                lua.load(&source)
                    .set_name(path.to_string_lossy().as_ref())
                    .exec()
                    .map_err(|e| anyhow::anyhow!("{e}"))?;

                log::debug!(path:% = path.display(); "loaded Lua rule file");
            }
        }

        Ok(Self { lua, rules })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

impl AccessMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "r",
            Self::Write => "w",
            Self::ReadWrite => "rw",
        }
    }
}

pub struct LuaRule {
    pub access: AccessMode,
    pub match_fn: mlua::RegistryKey,
}

#[derive(Clone)]
pub struct LuaDevice {
    pub device: udev::Device,
}

impl LuaDevice {
    pub const fn new(device: udev::Device) -> Self {
        Self { device }
    }
}

impl mlua::UserData for LuaDevice {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("subsystem", |_, this| {
            Ok(this
                .device
                .subsystem()
                .map(|subsystem| subsystem.to_string_lossy().into_owned()))
        });

        fields.add_field_method_get("sysname", |_, this| {
            Ok(this.device.sysname().to_string_lossy().into_owned())
        });

        fields.add_field_method_get("syspath", |_, this| {
            Ok(this.device.syspath().to_string_lossy().into_owned())
        });

        fields.add_field_method_get("devnode", |_, this| {
            Ok(this
                .device
                .devnode()
                .map(|devnode| devnode.to_string_lossy().into_owned()))
        });
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("attr", |_, this, name: String| {
            Ok(this
                .device
                .attribute_value(&name)
                .map(|value| value.to_string_lossy().into_owned()))
        });

        methods.add_method("property", |_, this, name: String| {
            Ok(this
                .device
                .property_value(&name)
                .map(|value| value.to_string_lossy().into_owned()))
        });

        methods.add_method("parent", |lua, this, subsystem: Option<String>| {
            let mut parent = this.device.parent();

            while let Some(device) = parent {
                let matches = subsystem.as_deref().is_none_or(|subsystem| {
                    device
                        .subsystem()
                        .is_some_and(|device_subsystem| device_subsystem == subsystem)
                });

                if matches {
                    return lua.create_userdata(Self::new(device)).map(Some);
                }

                parent = device.parent();
            }

            Ok(None::<mlua::AnyUserData>)
        });

        methods.add_method("parents", |lua, this, ()| {
            let parents = lua.create_table()?;
            let mut parent = this.device.parent();
            let mut index = 1;

            while let Some(device) = parent {
                parents.set(index, Self::new(device.clone()))?;
                index += 1;
                parent = device.parent();
            }

            Ok(parents)
        });
    }
}
