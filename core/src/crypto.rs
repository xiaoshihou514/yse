use argon2::Argon2;
pub use chacha20poly1305::Key;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use thiserror::Error;

const SALT: &[u8] = b"yse-argon2-salt-v1";

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("encryption failed: {0}")]
    Encrypt(String),
    #[error("decryption failed: {0}")]
    Decrypt(String),
    #[error("key derivation failed: {0}")]
    KeyDerive(String),
}

pub fn derive_key(password: &str) -> Result<Key, CryptoError> {
    let argon2 = Argon2::default();
    let mut key_bytes = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), SALT, &mut key_bytes)
        .map_err(|e| CryptoError::KeyDerive(e.to_string()))?;
    Ok(*Key::from_slice(&key_bytes))
}

pub fn encrypt(key: &Key, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| CryptoError::Encrypt(e.to_string()))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(key: &Key, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < 12 {
        return Err(CryptoError::Decrypt("data too short".into()));
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(key);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CryptoError::Decrypt(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let key = derive_key("test-password-123").unwrap();
        let original = b"hello yse!";
        let encrypted = encrypt(&key, original).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(original.to_vec(), decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = derive_key("password1").unwrap();
        let key2 = derive_key("password2").unwrap();
        let encrypted = encrypt(&key1, b"secret").unwrap();
        assert!(decrypt(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_short_data_fails() {
        let key = derive_key("test").unwrap();
        assert!(decrypt(&key, b"too short").is_err());
    }
}
