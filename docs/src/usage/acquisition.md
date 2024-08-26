# Acquisition

## Execute the collector

The collector is compiled for different operating systems and architectures. Each version is located in the `bin` directory. To make the execution easier, you can use the `run.ps1` script on Windows or the `run.sh` script on Linux/macOS. The scripts will automatically execute the collector for the current operating system.

```bash
ir-toolkit/
├── run.ps1
├── run.sh
└── bin/
    ├── windows/
    │   ├── keygen-x86_64-pc-windows-msvc.exe
    │   ├── unpacker-x86_64-pc-windows-msvc.exe
    │   └── collector-x86_64-pc-windows-msvc.exe
    ├── linux/...
    └── macos/...
```
The collector will then search for all definied workflow files. Each workflow that meets the launch condition for the current system will be executed.

![how_it_works](../assets/how_it_works.png "flowchart of how the collector works" =400x)
