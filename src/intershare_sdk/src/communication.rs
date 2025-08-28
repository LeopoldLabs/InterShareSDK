use crate::encryption::generate_iv;
use crate::encryption::EncryptedStream;
use log::info;
use prost_stream::Stream;
use protocol::communication::{EncryptionRequest, EncryptionResponse};
use rand_core::OsRng;
use rustls::pki_types::ServerName;
use std::error::Error;
use std::io::{Read, Write};
use std::ops::DerefMut;
use std::sync::Arc;
use x25519_dalek::{EphemeralSecret, PublicKey};


pub async fn initiate_sender_communication<'s, T>(
    stream: T,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, T>, Box<dyn Error>>
where
    T: Read + Write + 's,
{
    let config = rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
    .with_root_certificates(rustls::RootCertStore::empty())
    .with_no_client_auth();

    // TODO change to client name
    let server_name = ServerName::try_from("intershare")?;

    // TODO store in cell
    let config = Arc::new(config);

    let conn = rustls::ClientConnection::new(config, server_name)?;

    let tls = rustls::StreamOwned::new(conn, stream);

    return Ok(tls);
}

pub fn initiate_receiver_communication<T>(
    mut stream: T,
) -> Result<EncryptedStream<T>, Box<dyn Error>>
where
    T: Read + Write,
{
    let secret = EphemeralSecret::random_from_rng(OsRng);
    let public_key = PublicKey::from(&secret);

    let iv = generate_iv();

    let mut prost_stream = Stream::new(&mut stream);

    let encryption_request = match prost_stream.recv::<EncryptionRequest>() {
        Ok(message) => message,
        Err(error) => return Err(Box::new(error)),
    };

    let _ = prost_stream.send(&EncryptionResponse {
        public_key: public_key.as_bytes().to_vec(),
        iv: iv.to_vec(),
    });

    let public_key: [u8; 32] = encryption_request
        .public_key
        .try_into()
        .expect("Vec length is not 32");
    let foreign_public_key = PublicKey::from(public_key);

    let shared_secret = secret.diffie_hellman(&foreign_public_key);

    let encrypted_stream = EncryptedStream::new(shared_secret.to_bytes(), iv, stream);

    return Ok(encrypted_stream);
}
