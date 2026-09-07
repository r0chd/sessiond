---@meta sessiond-uaccess
---@class Device
---@field subsystem string?
---@field sysname string
---@field syspath string
---@field devnode string?
local Device = {}

---@param name string
---@return string?
function Device:attr(name) end

---@param name string
---@return string?
function Device:property(name) end

---@param subsystem? string
---@return Device?
function Device:parent(subsystem) end

---@return Device[]
function Device:parents() end

---@class Rule
---@field access '"r"'|'"w"'|'"rw"'
---@field match fun(dev: Device): boolean
---@param rule Rule
function device(rule) end

---@type integer
api_version = 1
