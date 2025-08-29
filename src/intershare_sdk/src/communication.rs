use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::ServerName;
use rustls::StreamOwned;
use std::error::Error;
use std::io::{Read, Write};

const PROTOCOL_VERSIONS: &[&'static rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

pub async fn initiate_sender_communication<'s, T>(
    stream: T,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, T>, Box<dyn Error>>
where
    T: Read + Write + 's,
{
    use rustls::{ClientConfig, ClientConnection};

    // TODO verify certificate GUI flow
    let provider = rustls::crypto::ring::default_provider();

    let config = ClientConfig::builder_with_provider(provider.into())
        .with_protocol_versions(PROTOCOL_VERSIONS)?
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();

    // TODO change to client name
    let server_name = ServerName::try_from("intershare")?;

    let conn = ClientConnection::new(config.into(), server_name)?;

    let tls = StreamOwned::new(conn, stream);

    return Ok(tls);
}

pub fn initiate_receiver_communication<T>(
    stream: T,
) -> Result<rustls::StreamOwned<rustls::ServerConnection, T>, Box<dyn Error>>
where
    T: Read + Write,
{
    use ring::signature::Ed25519KeyPair;
    use rustls::{pki_types::PrivateKeyDer, ServerConfig, ServerConnection};

    // TODO Store certificate
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())?;

    let key_der = PrivateKeyDer::from_pem_slice(pkcs8_bytes.as_ref())?;

    let provider = rustls::crypto::ring::default_provider();

    // TODO add client auth
    let config = ServerConfig::builder_with_provider(provider.into())
        .with_protocol_versions(PROTOCOL_VERSIONS)?
        .with_no_client_auth()
        .with_single_cert(Vec::new(), key_der)?;

    let conn = ServerConnection::new(config.into())?;

    let stream = StreamOwned::new(conn, stream);

    return Ok(stream);
}
