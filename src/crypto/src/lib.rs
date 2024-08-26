mod crypto_tests;
use config::workflow::Algorithm;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use openssl::pkey::{PKey, Public};
use openssl::rsa::{Padding, Rsa};
use openssl::sha::Sha1;
use openssl::symm::{Cipher, Crypter, Mode};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptionMeta {
    pub version: String,
    pub algorithm: Algorithm,
    #[serde(
        deserialize_with = "deserialize_vec_hex",
        serialize_with = "serialize_vec_hex"
    )]
    pub encrypted_key: Vec<u8>,
    #[serde(
        deserialize_with = "deserialize_vec_hex",
        serialize_with = "serialize_vec_hex"
    )]
    pub iv: Vec<u8>,
    #[serde(
        deserialize_with = "deserialize_vec_hex",
        serialize_with = "serialize_vec_hex"
    )]
    pub tag: Vec<u8>,
}
impl Default for EncryptionMeta {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            algorithm: Algorithm::None,
            encrypted_key: vec![],
            iv: vec![],
            tag: vec![],
        }
    }
}

fn deserialize_vec_hex<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // hex decode the string
    let s: String = serde::Deserialize::deserialize(deserializer)?;

    hex::decode(&s).map_err(serde::de::Error::custom)
}

fn serialize_vec_hex<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // hex encode the data
    serializer.serialize_str(&hex::encode(data))
}

/// Generate a symmetric key of the given size
pub fn generate_random(size: usize) -> Vec<u8> {
    let mut key = vec![0; size];
    openssl::rand::rand_bytes(&mut key).unwrap();
    key
}

pub fn load_private_key(
    private_key: PathBuf,
) -> Result<Rsa<openssl::pkey::Private>, Box<dyn Error>> {
    let mut private_key_file = File::open(private_key)?;
    let mut private_key_content = String::new();
    private_key_file.read_to_string(&mut private_key_content)?;
    let private_key = Rsa::private_key_from_pem(private_key_content.as_bytes())?;
    Ok(private_key)
}

pub fn load_public_key(public_key: PathBuf) -> Result<Rsa<openssl::pkey::Public>, Box<dyn Error>> {
    let mut public_key_file = match File::open(public_key) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open public key file: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut public_key_content = String::new();
    public_key_file.read_to_string(&mut public_key_content)?;

    let public_key = match Rsa::public_key_from_pem(public_key_content.as_bytes()) {
        Ok(key) => key,
        Err(e) => {
            error!("Failed to load public key: {}", e);
            return Err(Box::new(e));
        }
    };
    Ok(public_key)
}

pub fn generate_rsa_keypair(
    size: u32,
) -> Result<(PKey<openssl::pkey::Private>, PKey<openssl::pkey::Public>), Box<dyn std::error::Error>>
{
    let rsa = match openssl::rsa::Rsa::generate(size) {
        Ok(rsa) => rsa,
        Err(e) => {
            error!("Failed to generate RSA key pair: {}", e);
            return Err(Box::new(e));
        }
    };
    let private_key = PKey::from_rsa(rsa.clone())?;
    let public_key = PKey::from_rsa(openssl::rsa::Rsa::from_public_components(
        rsa.n().to_owned()?,
        rsa.e().to_owned()?,
    )?)?;
    Ok((private_key, public_key))
}

pub fn save_keypair(
    private_key: PKey<openssl::pkey::Private>,
    public_key: PKey<openssl::pkey::Public>,
    private_key_file: &String,
    public_key_file: &String,
) -> Result<(), Box<dyn std::error::Error>> {
    let private_key_pem = match private_key.private_key_to_pem_pkcs8() {
        Ok(pem) => pem,
        Err(e) => {
            error!("Failed to convert private key to PEM: {}", e);
            return Err(Box::new(e));
        }
    };
    let private_key_path = Path::new(private_key_file);
    let mut private_key_file = match File::create(&private_key_path) {
        Ok(file) => {
            debug!("Private key file created: {:?}", private_key_file);
            file
        }
        Err(e) => {
            error!("Failed to create private key file: {}", e);
            return Err(Box::new(e));
        }
    };
    match private_key_file.write_all(&private_key_pem) {
        Ok(_) => (),
        Err(e) => {
            error!("Failed to write private key to file: {}", e);
            return Err(Box::new(e));
        }
    };

    // Security check: the private key should not be inside the keys directory
    if private_key_path.parent().unwrap().ends_with("keys") {
        warn!("DO NOT store private keys in the keys directory. Make sure to store the private key in a secure location.");
    }

    let public_key_pem = match public_key.public_key_to_pem() {
        Ok(pem) => pem,
        Err(e) => {
            error!("Failed to convert public key to PEM: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut public_key_file = match File::create(Path::new(public_key_file)) {
        Ok(file) => {
            debug!("Public key file created: {:?}", public_key_file);
            file
        }
        Err(e) => {
            error!("Failed to create public key file: {}", e);
            return Err(Box::new(e));
        }
    };

    match public_key_file.write_all(&public_key_pem) {
        Ok(_) => (),
        Err(e) => {
            error!("Failed to write public key to file: {}", e);
            return Err(Box::new(e));
        }
    };

    Ok(())
}

/// Deserialize the metadata from the input .json file
pub fn get_metadata(input_path: &Path) -> Result<EncryptionMeta, Box<dyn std::error::Error>> {
    let metadata_path = input_path.with_extension("json");
    let metadata_file = File::open(metadata_path)?;
    let metadata: EncryptionMeta = serde_json::from_reader(metadata_file)?;
    Ok(metadata)
}

const BLOCK_SIZE: usize = 4096 * 4;

pub fn encrypt_evidence(
    output_path: &Path,
    public_key: Rsa<Public>,
    algorithm: Algorithm,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    // check if output file exists
    if !output_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File does not exist",
        )));
    }

    // check if algorithm is None
    if algorithm == Algorithm::None {
        warn!("Encryption algorithm is None: skipping encryption");
        return Ok((vec![], vec![], vec![]));
    }

    info!("Encrypting evidence file: {:?}", output_path);

    // Step 0: Initialize the sizes
    let block_size = algorithm.block_size();
    let key_size = algorithm.key_size();
    let iv_size = algorithm.iv_size();
    let tag_size = algorithm.tag_size();

    // Step 1: Generate a random key
    let mut key = generate_random(key_size);

    // Step 2: Encrypt the key using the public key
    let mut encrypted_key = vec![0; public_key.size() as usize];
    public_key.public_encrypt(&key, &mut encrypted_key, Padding::PKCS1)?;

    // Step 3: Initialize crypter and generate a random IV
    let cipher = match algorithm {
        Algorithm::AES128GCM => Cipher::aes_128_gcm(),
        Algorithm::CHACHA20POLY1305 => Cipher::chacha20_poly1305(),
        _ => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unsupported algorithm",
            )))
        }
    };
    let iv = generate_random(iv_size);
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, &key, Some(&iv))?;
    crypter.pad(false);

    // Step 4: Encrypt the file using the key in-place
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(output_path)?;

    file.seek(SeekFrom::Start(0))?;

    // Initialize progress bar
    let file_size = file.metadata()?.len();
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("=>-"),
    );

    let mut buffer = vec![0u8; block_size];
    let mut position = 0;
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        let mut ciphertext = vec![0; bytes_read];
        let count = crypter.update(&buffer[..bytes_read], &mut ciphertext)?;
        file.seek(SeekFrom::Start(position as u64))?;
        file.write_all(&ciphertext[..count])?;
        position += count;
        pb.set_position(position as u64);
    }
    pb.finish_and_clear();

    // Step 5: Finalize the encryption
    let mut final_buffer = vec![0; block_size];
    let count = crypter.finalize(&mut final_buffer)?;
    if count > 0 {
        file.seek(SeekFrom::Start(position as u64))?;
        file.write_all(&buffer[..count])?;
    }

    let mut tag = vec![0; tag_size];
    crypter.get_tag(&mut tag)?;

    // Step 6: Disallocate memory for key
    key.iter_mut().for_each(|b| *b = 0);

    Ok((encrypted_key, iv, tag))
}

pub fn decrypt_evidence(
    input_path: &Path,
    private_key: Rsa<openssl::pkey::Private>,
    metadata: EncryptionMeta,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if not algorithm is specified
    if metadata.algorithm == Algorithm::None {
        warn!("Encryption algorithm is None: skipping decryption");
        return Ok(());
    }

    // Step 0: Initialize the sizes
    let block_size = metadata.algorithm.block_size();
    let key_size = metadata.algorithm.key_size();

    // Step 1: Decrypt the key using the private key
    let mut key = vec![0; private_key.size() as usize];
    private_key.private_decrypt(&metadata.encrypted_key, &mut key, Padding::PKCS1)?;
    // change size of key to KEY_SIZE
    key = key.iter().cloned().take(key_size).collect();

    // Step 2: Initialize crypter and set the IV
    let cipher = match metadata.algorithm {
        Algorithm::AES128GCM => Cipher::aes_128_gcm(),
        Algorithm::CHACHA20POLY1305 => Cipher::chacha20_poly1305(),
        _ => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unsupported algorithm",
            )))
        }
    };
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, &key, Some(&metadata.iv))?;
    crypter.pad(false);

    // Step 3: Open the file and decrypt the content in-place
    let mut file = OpenOptions::new().read(true).write(true).open(input_path)?;

    // Initialize progress bar
    let file_size = file.metadata()?.len();
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    let mut buffer = vec![0u8; block_size];
    let mut position = 0;
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        let mut plaintext = vec![0; bytes_read];
        let count = crypter.update(&buffer[..bytes_read], &mut plaintext)?;
        file.seek(SeekFrom::Start(position as u64))?;
        file.write_all(&plaintext[..count])?;
        position += count;
        pb.set_position(position as u64);
    }
    pb.finish();

    // Step 4: Set the tag
    crypter.set_tag(&metadata.tag)?;

    // Step 5: Finalize the decryption and verify the tag
    // finalize will fail if the tag is invalid
    let count = match crypter.finalize(&mut buffer) {
        Ok(count) => count,
        Err(e) => {
            error!("Failed to finalize decryption: {}", e);
            return Err(Box::new(e));
        }
    };
    if count > 0 {
        file.seek(SeekFrom::Start(position as u64))?;
        file.write_all(&buffer[..count])?;
    }

    // Step 6: Disallocate memory for key
    key.iter_mut().for_each(|b| *b = 0);

    Ok(())
}

pub fn get_file_sha1(path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; BLOCK_SIZE];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:0>40}", hex::encode(hasher.finish())))
}

pub fn copy_file_with_sha1(
    src: &PathBuf,
    dest: &PathBuf,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut src_file = File::open(src)?;
    let mut dest_file = File::create(dest)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; BLOCK_SIZE];

    loop {
        let bytes_read = src_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest_file.write_all(&buffer[..bytes_read])?;
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:0>40}", hex::encode(hasher.finish())))
}
