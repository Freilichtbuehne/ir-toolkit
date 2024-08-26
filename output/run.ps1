# Detect the architecture and OS
$arch = (Get-WmiObject Win32_OperatingSystem).OSArchitecture.ToLower()
$os = (Get-WmiObject Win32_OperatingSystem).Caption.ToLower()

# Define the path to the collector binaries
$collectorPath = "bin\windows"

# Define the mapping for the collector executables based on the architecture
$collectorExecutable = ""

if ($os -like "*windows*") {
    switch ($arch) {
        "64-bit" {
            $collectorExecutable = "collector-x86_64-pc-windows-msvc.exe"
        }
        "32-bit" {
            $collectorExecutable = "collector-i686-pc-windows-msvc.exe"
        }
        "arm64" {
            $collectorExecutable = "collector-aarch64-pc-windows-msvc.exe"
        }
        # Add more cases as necessary for different architectures
        default {
            Write-Host "Unsupported architecture: $arch"
            Write-Host "Please manually run the binary in the bin directory"
            Pause
            exit 1
        }
    }
} else {
    Write-Host "Unsupported OS: $os"
    Write-Host "Please run the run.sh script in the same directory"
    Pause
    exit 1
}

# Construct the full path to the collector executable
$collectorFullPath = Join-Path -Path $collectorPath -ChildPath $collectorExecutable

# Check if the collector executable exists
if (-Not (Test-Path -Path $collectorFullPath)) {
    Write-Host "Binary not found: $collectorFullPath"
    Write-Host "Please manually run the binary in the bin directory"
    Pause
    exit 1
}

# Execute the collector
Write-Host "Executing collector: $collectorFullPath"
& $collectorFullPath

# End of script
