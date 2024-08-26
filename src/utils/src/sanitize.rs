use sanitize_filename;

/// Sanitize a directory name
pub fn sanitize_dirname(dirname: &str) -> String {
    let options = sanitize_filename::Options {
        truncate: true,
        // if windows is set to 'true', reserved windows filenames are also replaced
        // we always set it to 'true' to avoid any issues as analysis might be done on windows
        windows: true,
        replacement: "",
    };

    let sanitized = sanitize_filename::sanitize_with_options(dirname, options);
    sanitized.replace(" ", "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_dirname() {
        // this is an example for a report name
        assert_eq!(
            sanitize_dirname("MY-DEVICE_This is a dumb <> re*?port name!_2024-01-01_12-00-00"),
            "MY-DEVICE_This_is_a_dumb__report_name!_2024-01-01_12-00-00"
        );

        // example for path extraction of a report
        assert_eq!(sanitize_dirname("C:"), "C");
    }

    // test temp path for macOS that should get preserved
    #[test]
    fn test_sanitize_dirname_macos() {
        assert_eq!(
            sanitize_dirname("/var/folders/_x/x1x2x3x4x5x6x7x8x9x0x/T"),
            "varfolders_xx1x2x3x4x5x6x7x8x9x0xT"
        );
        assert_eq!(sanitize_dirname("/var/folders/m_/cksx93ys47x4621g0zbw_m4m0000gn/T/check_unpack_not_archived/test.txt"), "varfoldersm_cksx93ys47x4621g0zbw_m4m0000gnTcheck_unpack_not_archivedtest.txt");
    }
}
