use std::io::{Read, Write};
use rand_core::{OsRng, RngCore};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use crate::stream::Close;

pub fn generate_secure_base64_token(byte_length: usize) -> String {
    let mut bytes = vec![0u8; byte_length];
    OsRng.fill_bytes(&mut bytes);
    return URL_SAFE_NO_PAD.encode(&bytes)
}

pub trait EncryptedReadWrite: Read + Write + Send + Close {}
impl<T> EncryptedReadWrite for rustls::StreamOwned<rustls::ClientConnection, T> where T: Read + Write + Send + Close {}
impl<T> EncryptedReadWrite for rustls::StreamOwned<rustls::ServerConnection, T> where T: Read + Write + Send + Close {}