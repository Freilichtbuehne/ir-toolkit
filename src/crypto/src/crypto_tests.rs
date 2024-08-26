#[cfg(test)]
mod tests {

    use crate::*;
    use config::workflow::Algorithm;
    use log::debug;
    use openssl::sha::Sha256;
    use report::Report;
    use system::{get_base_path, SystemVariables};
    use utils::tests::Cleanup;

    #[test]
    fn check_encryption_decryption_aes() {
        let mut cleanup = Cleanup::new();

        // Step 1: Initialize report
        let mut system_variables = SystemVariables::new();
        let report = Report::new(
            &mut system_variables,
            true,
            "test_check_encryption_decryption_aes".to_string(),
        )
        .expect("Failed to initialize report");
        cleanup.add(report.dir.clone());

        debug!("Base path: {:?}", get_base_path());
        // Step 2: Generate key pair
        let (private_key, public_key) =
            generate_rsa_keypair(2048).expect("Failed to generate RSA key pair");

        // Step 3: Save key pair
        let private_key_file = report.loot_dir.join("private_key.pem");
        let public_key_file = report.loot_dir.join("public_key.pem");
        debug!("Private key file: {:?}", private_key_file);
        debug!("Public key file: {:?}", public_key_file);
        save_keypair(
            private_key,
            public_key,
            &private_key_file.to_str().unwrap().to_string(),
            &public_key_file.to_str().unwrap().to_string(),
        )
        .expect("Failed to save key pair");

        // Step 4: Load key pair
        let private_key = load_private_key(private_key_file).expect("Failed to load private key");
        let public_key = load_public_key(public_key_file).expect("Failed to load public key");

        // Step 5: Generate a 1MB file with random data
        let test_file = report.loot_dir.join("testfile.txt");
        let data = generate_random(1024 * 1024);
        std::fs::write(&test_file, &data).expect("Failed to write test file");

        // Step 6: Calculate the checksum of the data
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let pre_checksum = hasher.finish();

        // Step 7: Encrypt the file
        let algorithm = Algorithm::AES128GCM;
        let (encrypted_key, iv, tag) =
            encrypt_evidence(&test_file, public_key, algorithm).expect("Failed to encrypt file");

        let metadata = EncryptionMeta {
            version: "1.0".to_string(),
            algorithm: algorithm,
            encrypted_key,
            iv,
            tag,
        };

        // Step 8: Decrypt the file
        decrypt_evidence(&test_file, private_key, metadata).expect("Failed to decrypt file");

        // Step 9: Calculate the checksum of the decrypted data
        let decrypted_data = std::fs::read(&test_file).expect("Failed to read decrypted file");
        let mut hasher = Sha256::new();
        hasher.update(&decrypted_data);
        let post_checksum = hasher.finish();

        assert_eq!(pre_checksum, post_checksum, "Checksums do not match");
    }

    #[test]
    fn check_encryption_decryption_chacha() {
        let mut cleanup = Cleanup::new();

        // Step 1: Initialize report
        let mut system_variables = SystemVariables::new();
        let report = Report::new(
            &mut system_variables,
            true,
            "test_check_encryption_decryption_chacha".to_string(),
        )
        .expect("Failed to initialize report");
        cleanup.add(report.dir.clone());

        debug!("Base path: {:?}", get_base_path());
        // Step 2: Generate key pair
        let (private_key, public_key) =
            generate_rsa_keypair(2048).expect("Failed to generate RSA key pair");

        // Step 3: Save key pair
        let private_key_file = report.loot_dir.join("private_key.pem");
        let public_key_file = report.loot_dir.join("public_key.pem");
        debug!("Private key file: {:?}", private_key_file);
        debug!("Public key file: {:?}", public_key_file);
        save_keypair(
            private_key,
            public_key,
            &private_key_file.to_str().unwrap().to_string(),
            &public_key_file.to_str().unwrap().to_string(),
        )
        .expect("Failed to save key pair");

        // Step 4: Load key pair
        let private_key = load_private_key(private_key_file).expect("Failed to load private key");
        let public_key = load_public_key(public_key_file).expect("Failed to load public key");

        // Step 5: Generate a 1MB file with random data
        let test_file = report.loot_dir.join("testfile.txt");
        let data = generate_random(1024 * 1024);
        std::fs::write(&test_file, &data).expect("Failed to write test file");

        // Step 6: Calculate the checksum of the data
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let pre_checksum = hasher.finish();

        // Step 7: Encrypt the file
        let algorithm = Algorithm::CHACHA20POLY1305;
        let (encrypted_key, iv, tag) =
            encrypt_evidence(&test_file, public_key, algorithm).expect("Failed to encrypt file");

        let metadata = EncryptionMeta {
            version: "1.0".to_string(),
            algorithm: algorithm,
            encrypted_key,
            iv,
            tag,
        };

        // Step 8: Decrypt the file
        decrypt_evidence(&test_file, private_key, metadata).expect("Failed to decrypt file");

        // Step 9: Calculate the checksum of the decrypted data
        let decrypted_data = std::fs::read(&test_file).expect("Failed to read decrypted file");
        let mut hasher = Sha256::new();
        hasher.update(&decrypted_data);
        let post_checksum = hasher.finish();

        assert_eq!(pre_checksum, post_checksum, "Checksums do not match");
    }
}
