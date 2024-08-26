# Report

```yaml
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

## Archive

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `enabled`    | Specifies whether the zip archive creation is enabled.                      | No       | `true`  |
| `encryption` | Configuration for encrypting the zip archive. Contains the fields: `enabled`, `public_key`, and `algorithm`. | No | See `ReportingEncryption` Defaults |
| `compression`| Configuration for compressing the zip archive. Contains the fields: `enabled` and `size_limit`. | No | See `ReportingCompression` Defaults |

### Encryption

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `enabled`    | Specifies whether encryption is enabled for the zip archive.                | No       | `false` |
| `public_key` | The path to the public key file used for encryption. Relative to the `keys` directory | Yes (if `enabled` is `true`) | - |
| `algorithm`  | The encryption algorithm to be used. Available values: `AES-128-GCM`, `CHACHA20-POLY1305`, `None`. | No | `None` |

### Compression

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `enabled`    | Specifies whether compression is enabled for the zip archive.               | No       | `false` |
| `size_limit` | The maximum size limit for specific files to be compressed. If a file exceeds this limit, it will only be stored inside the archive without compression. | No | `100 MB` |

## Metadata

| Property     | Description                                                                 | Required | Default |
|--------------|-----------------------------------------------------------------------------|----------|---------|
| `mac_times`  | Specifies whether the MAC times (Modified, Accessed, Created) should be recorded in the `metadata.csv` for stored files (using `store` or `yara` actions). | No | `false` |
| `checksums`  | Specifies whether checksums should be calculated and included in the report. | No | `false` |
| `paths`      | Specifies whether the original file paths should be recorded in the `metadata.csv` for stored files (using `store` or `yara` actions). | No | `false` |
```