use clap::{Arg, Command};
use crypto::{generate_rsa_keypair, save_keypair};
use log::{error, info, LevelFilter};
use logging::Logger;
fn main() {
    let matches = get_command().get_matches();

    let logger = Logger::init()
        .set_level(match matches.get_flag("verbose") {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        })
        .apply();

    run(matches);

    logger.finish();
}

fn get_command() -> Command {
    Command::new("Keygen")
        .version("1.0")
        .about("Generates an RSA key pair")
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .value_name("SIZE")
                .help("The size of the RSA key")
                .value_parser(clap::value_parser!(u32))
                .default_value("2048"),
        )
        .arg(
            Arg::new("private_key")
                .short('p')
                .long("private")
                .value_name("PRIVATE_KEY")
                .required(true)
                .help("The filename for the private key (e.g. private_key.pem)"),
        )
        .arg(
            Arg::new("public_key")
                .short('u')
                .long("public")
                .value_name("PUBLIC_KEY")
                .required(true)
                .help("The filename for the public key (e.g. public_key.pem)"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enables verbose logging")
                .action(clap::ArgAction::SetTrue),
        )
}

fn run(matches: clap::ArgMatches) {
    let size: u32 = matches.get_one::<u32>("size").unwrap().clone();

    let private_key_file = matches.get_one::<String>("private_key").unwrap();
    let public_key_file = matches.get_one::<String>("public_key").unwrap();

    match generate_rsa_keypair(size) {
        Ok((private_key, public_key)) => {
            match save_keypair(private_key, public_key, private_key_file, public_key_file) {
                Ok(_) => info!("Successfully generated RSA key pair"),
                Err(e) => error!("Failed to save RSA key pair: {}", e),
            }
        }
        Err(e) => error!("Failed to generate RSA key pair: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;
    use std::fs;
    use std::path::Path;
    use utils::tests::Cleanup;

    fn test_command() -> Command {
        get_command()
    }

    fn assert_keys_exist_and_valid(private_key_path: &Path, public_key_path: &Path) {
        assert!(private_key_path.exists(), "Private key file does not exist");
        assert!(public_key_path.exists(), "Public key file does not exist");

        let private_key_content =
            fs::read_to_string(private_key_path).expect("Failed to read private key file");
        let public_key_content =
            fs::read_to_string(public_key_path).expect("Failed to read public key file");

        assert!(
            private_key_content.contains("-----BEGIN PRIVATE KEY-----"),
            "Invalid private key format"
        );
        assert!(
            public_key_content.contains("-----BEGIN PUBLIC KEY-----"),
            "Invalid public key format"
        );
    }

    #[test]
    fn test_keygen_command_with_defaults() {
        let mut cleanup = Cleanup::new();
        let temp_dir = cleanup.tmp_dir("test_keygen_command_with_defaults");
        let private_key_file = temp_dir.join("private_key.pem");
        let public_key_file = temp_dir.join("public_key.pem");

        let matches = test_command()
            .try_get_matches_from(vec![
                "keygen",
                "--private",
                private_key_file.to_str().unwrap(),
                "--public",
                public_key_file.to_str().unwrap(),
            ])
            .unwrap();

        run(matches);

        assert_keys_exist_and_valid(&private_key_file, &public_key_file);
    }

    #[test]
    fn test_keygen_command_with_custom_size() {
        let mut cleanup = Cleanup::new();
        let temp_dir = cleanup.tmp_dir("test_keygen_command_with_custom_size");

        let private_key_file = temp_dir.join("private_key.pem");
        let public_key_file = temp_dir.join("public_key.pem");

        let matches = test_command()
            .try_get_matches_from(vec![
                "keygen",
                "--size",
                "4096",
                "--private",
                private_key_file.to_str().unwrap(),
                "--public",
                public_key_file.to_str().unwrap(),
            ])
            .unwrap();

        run(matches);

        assert_keys_exist_and_valid(&private_key_file, &public_key_file);
    }

    #[test]
    fn test_keygen_command_invalid_size() {
        let mut cleanup = Cleanup::new();
        let private_key_file = cleanup.tmp_dir("private_key.pem").join("private_key.pem");
        let public_key_file = cleanup.tmp_dir("public_key.pem").join("public_key.pem");

        let result = test_command().try_get_matches_from(vec![
            "keygen",
            "--size",
            "invalid_size",
            "--private",
            private_key_file.to_str().unwrap(),
            "--public",
            public_key_file.to_str().unwrap(),
        ]);

        assert!(result.is_err(), "Command should fail with invalid size");
    }
}
