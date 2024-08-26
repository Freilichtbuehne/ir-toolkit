# Launch Conditions

```yaml
launch_conditions:
  os: ["windows"]
  enabled: false
  arch: ["x86", "x86_64", "aarch64", "arm"]
  is_elevated: false
  custom_command:
    cmd: "cmd"
    args: ["/c", "dir", "${USER_HOME}"]
    contains_any: ["Downloads", "Documents"]
```

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `os`         | The operating system(s) the workflow can be executed on. Available values: `windows`, `linux`, `macos`. | Yes       | - |
| `enabled`    | Can be used to disable a workflow. `false` will prevent the workflow from being executed, even if all other conditions are met. | No       | `true` |
| `arch`       | The architecture(s) the workflow can be executed on. Available values: `x86`, `x86_64`, `aarch64`, `arm`. | No      | `["x86", "x86_64", "aarch64", "arm"]` |
| `is_elevated`| If set to `true`, the workflow will only be executed if the user has elevated privileges. If set to `false`, it is not necessary to have elevated privileges. | No       | `false` |
| `custom_command`| Allows the execution of a custom command. The command is executed in the shell of the operating system. | No       | - |


## Custom Commands

This condition allows the execution of a custom command. The following properties are available:

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `cmd`        | The command to be executed.                                                  | Yes      | - |
| `args`       | The arguments for the command.                                               | No       | - |
| `contains_any`| The condition is met if at least one of the specified strings is found in the output of the command. | No      | - |
| `contains_all`| The condition is met if all of the specified strings are found in the output of the command. | No      | - |
| `contains_regex`| The condition is met if the regular expression is found in the output of the command. | No      | - |

You must specify at least one of the properties `contains_any`, `contains_all`, or `contains_regex`.

If you specify for example `contains_any: ["abc", "def"]` and `contains_all: ["ghi", "jkl"]`, the condition is met if both `contains_any` and `contains_all` are true. 
