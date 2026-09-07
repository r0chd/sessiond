-- Sound devices
device {
  access = "rw",
  match = function(dev)
    return dev.subsystem == "sound"
  end,
}
-- Webcams, frame grabber, TV cards
device {
  access = "rw",
  match = function(dev)
    return dev.subsystem == "video4linux"
      or dev.subsystem == "dvb"
      or dev.subsystem == "media"
      or dev.sysname == "udmabuf"
  end,
}

-- DRI video devices
device {
  access = "rw",
  match = function(dev)
    return dev.subsystem == "drm"
      and dev.sysname:match("^card.*$") ~= nil
  end,
}

-- DRI render nodes, DRI accel nodes, AMD KFD nodes, KVM
device {
  access = "rw",
  match = function(dev)
    return (
      dev.subsystem == "drm"
      and dev.sysname:match("^renderD.*$") ~= nil
    ) or (
      dev.subsystem == "accel"
      and dev.sysname:match("^accel.*$") ~= nil
    ) or (
      dev.subsystem == "kfd"
      and dev.sysname == "kfd"
    ) or (
      dev.subsystem == "misc"
      and dev.sysname == "kvm"
    )
  end,
}

-- rfkill / radio killswitches
device {
  access = "rw",
  match = function(dev)
    return dev.sysname == "rfkill"
      and dev.subsystem == "misc"
  end,
}

-- Optical drives
device {
  access = "rw",
  match = function(dev)
    if dev.subsystem ~= "scsi_generic" then
      return false
    end

    local scsi = dev:parent("scsi")
    if not scsi then
      return false
    end

    local ty = scsi:attr("type")
    return ty == "4" or ty == "5"
  end,
}

-- FIDO security keys
device {
  access = "rw",
  match = function(dev)
    if dev.subsystem ~= "hidraw" then
      return false
    end
    for _, parent in ipairs(dev:parents()) do
      local product = parent:property("PRODUCT")
      if product ~= nil and product:match("^1050/") ~= nil then
        return true
      end
    end
    return false
  end,
}
