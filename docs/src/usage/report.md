# Report

Each workflow creates one report. You can specify the report format in the workflow configuration. This includes:
- Enabling or disabling the ZIP compression
- Enabling or disabling the encryption
- Specifying the encryption algorithm
- Metadata to collect (MAC times, checksums)

The console output of the collector is seperate from the reports and is stored in the `/reports` directory as a `.log` file.

## Report structure

The report is structured as follows:

```plaintext
reports/
└── MYPC_Windows_Example_2024-08-12_13-45-20/
    ├── action_output/...
    ├── loot_files/...
    ├── store_files/...
    └── metadata.csv
```

- `action_output/`: Contains the output of each action in the workflow (for example `stdout` and `stderr`).
- `loot_files/`: Contains all files you placed there manually during the workflow. This should be the output directory for your disk images or memory dumps. 
- `store_files/`: Contains all files that were stored using the `store` or `yara` action. Filenames are replaced with their SHA256 hash.
- `metadata.csv`: Contains the metadata of all files in the `store_files` directory. The metadata includes the SHA256 hash, the file path, the file size, and the MAC times (modified, accessed, created), etc.

If the report is encrypted, everything inside the report directory is archived in a `report.zip` file. The `encryption.json` file contains the encryption algorithm and the (encrypted) symmetric key:

```plaintext
reports/
└── MYPC_Windows_Example_2024-08-12_13-45-20/
    ├── report.zip
    └── encryption.json
```


## 1. Locate the generated report

The generated report is located in the `/reports` directory.


## 2. Unpacking/Decrypion

The `unpacker` tool, which is located in the `bin` directory, automatically detects if the report was encrypted or archived.

Run the `unpacker` tool with the `--help` flag to see the available options.

### 2.1. Unpacking a report without encryption or compression


```bash
[unpacker-binary].exe -i reports/MYPC_Example_2024-08-12_13-45-20 --restore --verify
```

This will do the following:
1. All stored files (using the `store` or `yara` action) will be restored by recreating the original file structure in the report directory. This does not apply to files that w
2. The integrity of all files in the `store_files` directory will be verified using the metadata in the `metadata.csv` file.


### 2.2. Unpacking a report with compression and encryption 

```bash
[unpacker-binary].exe -i reports/MYPC_Example_2024-08-12_13-45-20 -k key/private_key.pem --restore --verify
```

This will do the following:
1. The `report.zip` will be decrypted using the private key specified with the `-k` flag. The process will fail if the file was tampered with or the key is incorrect.
2. The `report.zip` file will be extracted to the report directory.
3. All stored files (using the `store` or `yara` action) will be restored by recreating the original file structure in the report directory.
4. The integrity of all files in the `store_files` directory will be verified using the metadata in the `metadata.csv` file.