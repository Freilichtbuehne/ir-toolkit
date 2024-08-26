# Examples

## Windows

```yaml
properties:
  title: "Windows Example"
  description: "This is an example configuration file for Windows"
  author: "John Doe"
  version: "1.0"

launch_conditions:
  os: ["windows"]
  enabled: true
  arch: ["x86", "x86_64"]
  is_elevated: true

actions:
  - name: disable_network
    type: command
    attributes:
      cmd: "cmd"
      args: ["/c", "${CUSTOM_FILES_DIR}\\disable_network.bat"]
      log_to_file: true

  - name: memory_dump
    type: binary
    attributes:
      path: "winpmem_mini_${ARCH}.exe"
      args: ["${LOOT_DIR}\\${DEVICE_NAME}.dmp"]
      log_to_file: true

  - name: clipboard
    type: command
    attributes:
      cmd: "powershell"
      args: ["-c", "Get-Clipboard"]
      log_to_file: true

  - name: disk_image
    type: binary
    attributes:
      path: "ftk/FTK Imager.exe"
      log_to_file: true

  - name: registry_dump
    type: command
    attributes:
      cmd: "cmd"
      args: ["/c", "reg save HKLM\\SYSTEM ${LOOT_DIR}\\registry_dump.reg"]
      log_to_file: true

  - name: event_log
    type: command
    attributes:
      cmd: "powershell"
      args: ["-c", "wevtutil epl System ${LOOT_DIR}\\system.evtx"]
      log_to_file: true

  - name: browser
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        ${USER_HOME}/AppData/Local/Google/Chrome/User Data/Default/History
        ${USER_HOME}/AppData/Roaming/Mozilla/Firefox/Profiles/*/places.sqlite
      size_limit: 5 GB

workflow:
  - action: disable_network
  - action: memory_dump
  - action: clipboard
  - action: disk_image
  - action: registry_dump
  - action: event_log
  - action: browser

reporting:
  zip_archive:
    enabled: false
    encryption:
      enabled: false
      public_key: "example_public.pem"
      algorithm: CHACHA20-POLY1305
    compression:
      enabled: true
      size_limit: 100 MB
  metadata:
    mac_times: true
    checksums: true
    paths: true
```

## Linux

```yaml
properties:
  title: "Linux Example"
  description: "This is an example configuration file for Linux"
  author: "John Doe"
  version: "1.0"

launch_conditions:
  os: ["linux"]
  enabled: true
  arch: ["x86", "x86_64"]
  is_elevated: true

actions:
  - name: disable_network
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "ip link set $(ip route get 1.1.1.1 | awk '{print $5; exit}') down"]
      log_to_file: true

  - name: memory_dump
    type: binary
    attributes:
      path: "dumpitforlinux"
      args: ["-v", "${LOOT_DIR}/${DEVICE_NAME}.dmp"]
      log_to_file: true

  - name: activities
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "ps aux > ${LOOT_DIR}/processes.txt && netstat -tulnp > ${LOOT_DIR}/network_connections.txt && lsof > ${LOOT_DIR}/open_files.txt"]
      log_to_file: true

  - name: disk_image
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "${CUSTOM_FILES_DIR}/dc3dd if=$(df / | tail -1 | awk '{print $1}') of=${LOOT_DIR}/root_partition.img"]
      log_to_file: true

  - name: browser
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        /home/*/.mozilla/firefox/*.default-release/places.sqlite
        /home/*/.config/google-chrome/Default/History
      size_limit: 5 GB

workflow:
  - action: disable_network
  - action: memory_dump
  - action: activities
  - action: disk_image
  - action: browser

reporting:
  zip_archive:
    enabled: false
    encryption:
      enabled: false
      public_key: "example_public.pem"
      algorithm: CHACHA20-POLY1305
    compression:
      enabled: true
      size_limit: 100 MB
  metadata:
    mac_times: true
    checksums: true
    paths: true
```

## macOS

```yaml
properties:
  title: "macOS Example"
  description: "This is an example configuration file for macOS"
  author: "John Doe"
  version: "1.0"

launch_conditions:
  os: ["macos"]
  enabled: true
  arch: ["x86", "x86_64", "aarch64", "arm"]
  is_elevated: true

actions:
  - name: disable_network
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "networksetup -setairportpower en0 off"]
      log_to_file: true

  - name: activities
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "ps -e > ${LOOT_DIR}/processes.txt && lsof -i > ${LOOT_DIR}/open_files.txt"]
      log_to_file: true

  - name: disk_image
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "hdiutil create -srcdevice $(df / | tail -1 | awk '{print $1}') -format UDRW ${LOOT_DIR}/disk_image.dmg"]
      log_to_file: true

  - name: email_chat
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        /Users/*/Library/Mail/**/*
        /Users/*/Library/Messages/**/*
      size_limit: 5 GB

  - name: browser
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        /Users/*/Library/Safari/History.db
        /Users/*/Library/Application Support/Google/Chrome/Default/History
      size_limit: 5 GB

workflow:
  - action: disable_network
  - action: activities
  - action: disk_image
  - action: email_chat
  - action: browser

reporting:
  zip_archive:
    enabled: false
    encryption:
      enabled: false
      public_key: "example_public.pem"
      algorithm: CHACHA20-POLY1305
    compression:
      enabled: true
      size_limit: 100 MB
  metadata:
    mac_times: true
    checksums: true
    paths: true
```