// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Standard binary age with a deliberately narrow passphrase profile. Admission
//! checks constrain untrusted headers before the age parser or KDF is invoked.

use crate::project_document::{MAX_JSON_BYTES, ProjectError};
use age::{
    scrypt,
    secrecy::{ExposeSecret, SecretString},
};
use std::io::{self, Read, Write};
use zeroize::Zeroizing;

pub(crate) const MAX_FILE_BYTES: usize = 384 * 1024;
pub(crate) const MAX_HEADER_BYTES: usize = 4 * 1024;
pub(crate) const SCRYPT_LOG_N: u8 = 18;
pub(crate) const MAX_PASSPHRASE_BYTES: usize = 1024;

pub(crate) fn validate_passphrase(phrase: &str) -> Result<(), ProjectError> {
    if phrase.is_empty() || phrase.len() > MAX_PASSPHRASE_BYTES || phrase.contains(['\r', '\n']) {
        Err(ProjectError::InvalidPassphrase)
    } else {
        Ok(())
    }
}

// This is format admission, not a cryptographic parser. age remains responsible
// for canonical salt/body parsing, header authentication, and payload AEAD.
fn check_header(ciphertext: &[u8]) -> Result<(), ProjectError> {
    if ciphertext.len() > MAX_FILE_BYTES {
        return Err(ProjectError::BudgetExceeded);
    }
    let header_limit = ciphertext.len().min(MAX_HEADER_BYTES);
    let mut lines = ciphertext[..header_limit].split_inclusive(|byte| *byte == b'\n');
    if lines.next() != Some(b"age-encryption.org/v1\n".as_slice()) {
        return Err(ProjectError::AuthenticationFailed);
    }
    let mut stanza_count = 0;
    for line in lines {
        if !line.ends_with(b"\n") {
            break;
        }
        if line.starts_with(b"--- ") {
            return if stanza_count == 1 {
                Ok(())
            } else {
                Err(ProjectError::AuthenticationFailed)
            };
        }
        if line.starts_with(b"-> ") {
            stanza_count += 1;
            let parts: Vec<&[u8]> = line
                .strip_suffix(b"\n")
                .unwrap()
                .split(|byte| *byte == b' ')
                .collect();
            if stanza_count != 1
                || parts.len() != 4
                || parts[0] != b"->"
                || parts[1] != b"scrypt"
                || parts[3] != b"18"
            {
                return Err(ProjectError::AuthenticationFailed);
            }
        }
    }
    Err(if ciphertext.len() >= MAX_HEADER_BYTES {
        ProjectError::BudgetExceeded
    } else {
        ProjectError::AuthenticationFailed
    })
}

struct CiphertextWriter(Vec<u8>);
impl Write for CiphertextWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FILE_BYTES - self.0.len() {
            return Err(io::Error::other("project budget"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The command owner admits exactly one worker without a waiting queue. Keep
/// authentication and re-encryption sequential within that operation.
pub(crate) fn encrypt(plaintext: &[u8], phrase: SecretString) -> Result<Vec<u8>, ProjectError> {
    validate_passphrase(phrase.expose_secret())?;
    if plaintext.len() > MAX_JSON_BYTES {
        return Err(ProjectError::BudgetExceeded);
    }
    let mut recipient = scrypt::Recipient::new(phrase);
    recipient.set_work_factor(SCRYPT_LOG_N);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| ProjectError::CryptoFailed)?;
    let output = CiphertextWriter(Vec::new());
    let mut writer = encryptor
        .wrap_output(output)
        .map_err(|_| ProjectError::CryptoFailed)?;
    writer
        .write_all(plaintext)
        .map_err(|_| ProjectError::CryptoFailed)?;
    let encrypted = writer.finish().map_err(|_| ProjectError::CryptoFailed)?.0;
    check_header(&encrypted).map_err(|_| ProjectError::CryptoFailed)?;
    Ok(encrypted)
}

pub(crate) fn decrypt(
    ciphertext: &[u8],
    phrase: SecretString,
) -> Result<Zeroizing<Vec<u8>>, ProjectError> {
    validate_passphrase(phrase.expose_secret())?;
    check_header(ciphertext)?;
    let decryptor =
        age::Decryptor::new_buffered(ciphertext).map_err(|_| ProjectError::AuthenticationFailed)?;
    if !decryptor.is_scrypt() {
        return Err(ProjectError::AuthenticationFailed);
    }
    let mut identity = scrypt::Identity::new(phrase);
    identity.set_max_work_factor(SCRYPT_LOG_N);
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| ProjectError::AuthenticationFailed)?;
    read_plaintext(reader)
}

fn read_plaintext(mut reader: impl Read) -> Result<Zeroizing<Vec<u8>>, ProjectError> {
    let mut plaintext = Zeroizing::new(Vec::new());
    // Fixed chunks avoid read_to_end's speculative growth. Consume authenticated
    // EOF before returning any bytes, including for an exact-size payload.
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        let remaining = MAX_JSON_BYTES - plaintext.len();
        let capacity = chunk.len().min(remaining + 1);
        let count = reader
            .read(&mut chunk[..capacity])
            .map_err(|_| ProjectError::AuthenticationFailed)?;
        if count == 0 {
            break;
        }
        if count > remaining {
            return Err(ProjectError::BudgetExceeded);
        }
        plaintext.extend_from_slice(&chunk[..count]);
    }
    Ok(plaintext)
}

#[cfg(test)]
pub(crate) static CRYPTO_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    fn phrase(value: &str) -> SecretString {
        SecretString::from(value.to_owned())
    }
    #[test]
    fn phrase_preserves_unicode_and_spaces_and_rejects_only_the_contract() {
        assert!(validate_passphrase(" é \t ").is_ok());
        assert!(validate_passphrase(&"x".repeat(1024)).is_ok());
        for invalid in ["", "line\nline", "line\rline", &"x".repeat(1025)] {
            assert_eq!(
                validate_passphrase(invalid),
                Err(ProjectError::InvalidPassphrase)
            );
        }
    }
    #[test]
    fn header_admission_rejects_oversize_work_factor_recipients_and_armor_without_a_kdf() {
        let valid = b"age-encryption.org/v1\n-> scrypt salt 18\nbody\n--- mac\n";
        assert!(check_header(valid).is_ok());
        for bytes in [
            b"age-encryption.org/v1\n-> scrypt salt 19\nbody\n--- mac\n".as_slice(),
            b"age-encryption.org/v1\n-> scrypt salt 17\nbody\n--- mac\n",
            b"age-encryption.org/v1\n-> scrypt salt 18\nbody\n-> scrypt salt 18\nbody\n--- mac\n",
            b"age-encryption.org/v1\n-> X25519 key\nbody\n--- mac\n",
            b"-----BEGIN AGE ENCRYPTED FILE-----\n",
        ] {
            assert_eq!(check_header(bytes), Err(ProjectError::AuthenticationFailed));
        }
        let mut oversized = b"age-encryption.org/v1\n-> scrypt salt 18\n".to_vec();
        oversized.resize(MAX_HEADER_BYTES, b'x');
        assert_eq!(check_header(&oversized), Err(ProjectError::BudgetExceeded));
        assert_eq!(
            check_header(&vec![0; MAX_FILE_BYTES + 1]),
            Err(ProjectError::BudgetExceeded)
        );
        let mut exact = b"age-encryption.org/v1\n-> scrypt salt 18\n".to_vec();
        exact.resize(MAX_HEADER_BYTES - b"\n--- mac\n".len(), b'x');
        exact.extend_from_slice(b"\n--- mac\n");
        assert_eq!(exact.len(), MAX_HEADER_BYTES);
        assert!(check_header(&exact).is_ok());
        exact.insert(0, b' ');
        assert!(check_header(&exact).is_err());
    }

    #[test]
    fn decrypted_buffer_is_bounded_and_eof_errors_never_return_partial_bytes() {
        assert_eq!(
            read_plaintext(std::io::Cursor::new(vec![b'x'; MAX_JSON_BYTES]))
                .unwrap()
                .len(),
            MAX_JSON_BYTES
        );
        assert_eq!(
            read_plaintext(std::io::Cursor::new(vec![b'x'; MAX_JSON_BYTES + 1])),
            Err(ProjectError::BudgetExceeded)
        );
        struct FailsAtEof(bool);
        impl Read for FailsAtEof {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("synthetic read failure"));
                }
                self.0 = true;
                buffer[0] = b'x';
                Ok(1)
            }
        }
        assert_eq!(
            read_plaintext(FailsAtEof(false)),
            Err(ProjectError::AuthenticationFailed)
        );
        assert_eq!(
            encrypt(&vec![0; MAX_JSON_BYTES + 1], phrase("Synthetic phrase")),
            Err(ProjectError::BudgetExceeded)
        );
    }
    #[test]
    fn standard_age_round_trip_authentication_tampering_and_truncation() {
        let _guard = CRYPTO_TEST_LOCK.lock().unwrap();
        let plaintext = b"Synthetic authoring bytes\n\t\n";
        let encrypted = encrypt(plaintext, phrase("Synthetic test phrase only")).unwrap();
        assert!(encrypted.starts_with(b"age-encryption.org/v1\n-> scrypt "));
        assert!(encrypted.windows(4).any(|w| w == b" 18\n"));
        assert_eq!(
            decrypt(&encrypted, phrase("Synthetic test phrase only"))
                .unwrap()
                .as_slice(),
            plaintext
        );
        assert_eq!(
            decrypt(&encrypted, phrase("Different synthetic phrase")),
            Err(ProjectError::AuthenticationFailed)
        );
        let mut tampered = encrypted.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            decrypt(&tampered, phrase("Synthetic test phrase only")),
            Err(ProjectError::AuthenticationFailed)
        );
        assert_eq!(
            decrypt(
                &encrypted[..encrypted.len() - 1],
                phrase("Synthetic test phrase only")
            ),
            Err(ProjectError::AuthenticationFailed)
        );
        let mut trailing = encrypted;
        trailing.push(0);
        assert_eq!(
            decrypt(&trailing, phrase("Synthetic test phrase only")),
            Err(ProjectError::AuthenticationFailed)
        );
    }
}
