# Variables

The collector replaced all `${VAR_NAME}` variables with the actual values during the execution.

You can use variables in the following places:
1. All string values (including lists) in the `actions` section
2. All string values in the `custom_command` section of the `launch_conditions` section


**Example:**

```yaml
  - name: memory_dump
    type: binary
    attributes:
      path: "winpmem_mini_${ARCH}.exe"
      args: ["${LOOT_DIR}\\${DEVICE_NAME}.dmp"]
```


## Available Variables

| Variable Name | Description | Example |
|---------------|-------------|---------|
| `BASE_PATH` | The base path where the application stores its data. | `E:/collector/` |
| `DEVICE_NAME` | The name of the device. | `DESKTOP-1234` |
| `USER_HOME` | The path to the user's home directory. | `C:/Users/JohnDoe` |
| `USER_NAME` | The name of the user. | `JohnDoe` |
| `LOOT_DIR` | The path to the loot directory. | `E:/collector/reports/[NAME]/loot_files/` |
| `CUSTOM_FILES_DIR` | The path to the custom files directory. | `E:/collector/custom_files/` |
| `OS` | The operating system. | `windows` |
| `ARCH` | The architecture. | `x86_64` |