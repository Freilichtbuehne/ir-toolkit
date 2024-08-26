# Actions

You can define all actions and use them in the [next chapter](workflow.md).
The defined actions are not executed until they are used in the workflow.

```yaml
actions:
  - name: whoami
    type: command
    attributes:
      cmd: "cmd"
      args: ["/c", "whoami"]
      log_to_file: true

  - name: executables
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        ${USER_HOME}/Downloads/**/*.exe
      size_limit: 10 GB

  - name: memory_dump
    type: binary
    attributes:
      path: "winpmem_mini_${ARCH}.exe"
      args: ["${LOOT_DIR}\\${DEVICE_NAME}.dmp"]
      log_to_file: true
```

The variables `${USER_HOME}`, `${LOOT_DIR}`, `${DEVICE_NAME}`, and `${ARCH}` are replaced with the actual values during the execution of the collector. See the [variables](variables.md) section for more information.

## Available Actions

| Action Type | Description |
|-------------|-------------|
| `command`   | Execute a command |
| `binary`    | Executes a binary. The path is relative to the `custom_files` directory. But you can also use absolute paths. |
| `store`     | Store files that match a pattern. The pattern can be a glob pattern or a regular expression. See [glob](https://docs.rs/glob/latest/glob/) for more information. |
| `yara`      | Store files that match a YARA rule. You might place them in the `custom_files` directory. The files to scan do also use glob patterns. |
| `terminal` | Open a terminal window to execute arbitrary commands. A transcript of the terminal session is stored in the `action_output` directory of the report. |

**Hint:** For glob patterns, path separators (`/` and `\\`) are valid on all operating systems.

### 1. Command

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `cmd`        | The command to be executed.                                                  | Yes      | - |
| `args`       | The arguments for the command.                                               | No       | `[]` |
| `cwd`        | The working directory from which the command is executed.                    | No       | `""` (empty string) |
| `log_to_file`| If set to `true`, the output of the command will be logged to a file.        | No       | `true` |

**Example:**

```yaml
  - name: disable_network
    type: command
    attributes:
      cmd: "sh"
      args: ["-c", "ip link set $(ip route get 1.1.1.1 | awk '{print $5; exit}') down"]
      log_to_file: true
```

### 2. Binary

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `path`       | The path to the binary file to be executed.                                  | Yes      | - |
| `args`       | The arguments for the binary file.                                           | No       | `[]` |
| `log_to_file`| If set to `true`, the output of the binary execution will be logged to a file.| No       | `true` |

**Example:**

```yaml
  - name: memory_dump
    type: binary
    attributes:
      path: "dumpitforlinux"
      args: ["-v", "${LOOT_DIR}/${DEVICE_NAME}.dmp"]
      log_to_file: true
```

### 3. Store

| Property        | Description                                                               | Required | Default |
|-----------------|---------------------------------------------------------------------------|----------|---------|
| `case_sensitive`| If set to `true`, the pattern matching will be case-sensitive.             | No       | `true` |
| `patterns`      | The file patterns or paths to be matched and stored. Multiple patterns can be specified using new lines. | Yes      | - |
| `size_limit`    | The size limit for the files to be stored. The value should be specified in bytes. | No       | `Unlimited` |

**Example:**

```yaml
  - name: browser
    type: store
    attributes:
      case_sensitive: false
      patterns: |
        /home/*/.mozilla/firefox/*.default-release/places.sqlite
        /home/*/.config/google-chrome/Default/History
      size_limit: 5 GB
```

### 4. Terminal

| Property            | Description                                                               | Required | Default |
|---------------------|---------------------------------------------------------------------------|----------|---------|
| `shell`             | The shell to be used for executing the command.                            | No       | Will use the default shell of the operating system. |
| `wait`              | If set to `true`, the workflow will wait for the terminal to be closed.   | No       | `false` |
| `separate_window`   | If set to `true`, a terminal window will be opened. If set to `false`, an interactive shell will be opened in the current terminal. | No       | `true` |
| `enable_transcript` | If set to `true`, the output of the terminal will be captured and stored. This uses the `script` command on Linux and macOS and the `Start-Transcript` cmdlet on Windows. | No       | `true` |

**Note:**
- On Windows the `conhost` process will be opened in a separate window.
- On macOS the `Terminal.app` will be opened in a separate window.
- On Linux a list of known terminal apps will be checked and the first one found will be used. If no known terminal app is found, the default shell will be used and the `separate_window` property will be ignored.

There are some limitations when using the `terminal` action:
- If the `wait` property is `true`, then `separate_window` must also be set to `true`.
- If the `wait` property is `false`, then `enable_transcript` must also be set to `false`. This is because the workflow might have already finishes when the transcript file will be saved.

**Example:**

```yaml
  - name: terminal
    type: terminal
    attributes:
      shell: "bash"
      wait: true
      separate_window: true
      enable_transcript: true
```

### 5. Yara

| Property        | Description                                                               | Required | Default |
|-----------------|---------------------------------------------------------------------------|----------|---------|
| `rules_paths`   | The path to the Yara rules file(s). Multiple paths can be specified using new lines. The paths are relative to the `custom_files` directory. | Yes      | - |
| `files_to_scan` | The files or directories to be scanned. Multiple paths can be specified using new lines. | Yes      | - |
| `store_on_match`| If set to `true`, any matches found will be stored.                        | No       | `true` |
| `num_threads`   | The number of threads to be used for the scan.                             | No       | `1` |
| `scan_timeout`  | The maximum time allowed for the scan, in seconds.                         | No       | `60` |


**Example:**

```yaml
  - name: pdf_files
    type: yara
    attributes:
      rules_paths: |
        yara/*.yara
      files_to_scan: |
        ${USER_HOME}/Downloads/**/*
      store_on_match: true
      scan_timeout: 4s
```