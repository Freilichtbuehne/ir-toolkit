# Configuration


## 1. Define your workflows

Workflows are defined in the `/workflows` directory. You can use the provided examples as a starting point.

```bash
ir-toolkit/
└── workflows/
    ├── workflow1.yaml
    └── workflow2.yaml
```

You can also structure your workflows in subdirectories. The toolkit will recursively search for workflows in the `/workflows` directory.

See the [workflow chapter](../workflow/function.md) for more information on how to define workflows.

## 2. Use custom tools

Tools like Autoruns or FTK Imager can be placed in the `custom_files` directory. Within the workflow, you can run these tools by specifying the relative path to the executable.

For example, if you have `custom_files/windows_tools/autorunsc.exe`, you can use the following action in your workflow:
```yaml
- name: autorunsc
  type: binary
  attributes:
    path: "windows_tools/autorunsc.exe"
```

*Note:* You can use both `\\` and `/` as path separators.

## 3. Configure the toolkit

The configuration file `config.yaml` is located in the root directory of the toolkit. The settings apply to all workflows. You can adjust the following settings:

```yaml
time:
  ## The time zone to use for the timestamps in the report.
  ## e.g. "UTC", "Europe/Berlin", "Etc/GMT+2" or "UTC"
  ## For a list of time zones see: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones
  time_zone: "UTC"

  ## Enable NTP time to ensure that the system time is correct.
  ## WARNING: Enabling NTP time will delay the start of the workflow
  ##
  ## According to Federal Office for Information Security (BSI) in Germany,
  ##   changing the system time itself, e.g. to cover tracks, can be an incident
  ##   to be verified, both the hardware-based time from the RTC and the system
  ##   time must be recorded and compared with one from an independent time source.
  ## See: https://www.bsi.bund.de/EN/Themen/Oeffentliche-Verwaltung/Sicherheitspruefungen/IT-Forensik/forensik_node.html
  ntp_enabled: false
  ## Time in seconds to wait for an NTP server to respond.
  ## If the NTP server does not respond within this time,
  ##   the next server in ntp_servers will be tried.
  ## If set to 0, no timeout is used.
  ntp_timeout: 2
  ntp_servers: ["0.pool.ntp.org:123", "1.pool.ntp.org:123"]

## If set to true, the collector will attempt to elevate its privileges
## If set to false, the collector will run with the privileges of the user executing it
elevate: false
```

## 4. (Optional) Generate a new public/private key pair

If you want authenticated encryption for the report, you can generate a new public/private key pair using the `keygen` tool, which is located in the `bin` directory.

```bash
[keygen-binary].exe --private private_key.pem --public public_key.pem --size 2048
```

Move the public key to the `/keys` directory and reference it in the workflow.

```yaml
reporting:
  zip_archive:
    enabled: true
    encryption:
      enabled: true
      public_key: "example_public.pem"
      algorithm: CHACHA20-POLY1305
```

**Warning:** Do not put the private key in the toolkit directory. Keep it in a secure location.

The encrypted report can be decrypted using the `unpacker` tool, which is also located in the `bin` directory.

See the [report chapter](../usage/report.md) for more information on how to generate and locate the report.
