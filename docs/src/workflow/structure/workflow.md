# Workflow

```yaml
workflow:
  - action: disable_network
  - action: memory_dump
    timeout: 10m
    on_error: abort
  - action: activities
    on_error:
      goto: browser
  - action: disk_image
    timeout: 10s
  - action: browser
```

The `workflow` section defines the order in which the actions are executed. Each action is defined by its name and can have additional properties like `timeout` and `on_error`.

**Hint:** All actions must be defined in the [actions](actions.md) section.

## Action Properties

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `action`     | The name of the action to be executed.                                      | Yes      | -       |
| `timeout`    | The maximum time the action is allowed to run. Avaliable for `command` and `binary` actions. | No       | -       |
| `on_error`   | The action to be executed if an error occurs.                                | No       | `continue` |
| `parallel`   | This action will run in the background. The next action will be executed immediately. If the workflow finishes, the collector will wait for the parallel actions to finish before creating the report. Available for `command`, `binary` and `terminal` actions. | No       | `false` |

## Error Handling

The `on_error` property defines what should happen if an error occurs during the execution of an action. An action is considered to have failed if:
- A childprocess returns a non-zero exit code.
- A timeout occurs.
- An error occurs during the execution of the action.

The following options are available:
- `continue`: Continue with the next action. This is the default behavior.
- `abort`: Stop the workflow and do not execute any further actions. This will wait for all parallel actions to finish before creating the report.
- `goto`: Jump to a specific action. The action must be defined in the workflow. This is useful if you want to skip actions that are not necessary in case of an error.

**Example:**

```yaml
workflow:
  - action: 1
    on_error: continue
  - action: 2
    on_error:
      goto: 4
  - action: 3
    on_error: abort
  - action: 4
```