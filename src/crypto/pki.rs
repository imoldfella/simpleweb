use std::fs::File;

use std::sync::Arc;

use rustls::pki_types::CertificateDer;
use rustls::ServerConfig;

use std::io::BufReader;

pub fn load_tls_config() -> Arc<ServerConfig> {
    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let certs: Vec<CertificateDer> = rustls_pemfile::certs(cert_file)
        .collect::<Result<_, _>>()
        .unwrap();
    let keys = rustls_pemfile::private_key(key_file).unwrap().unwrap();

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, keys)
        .map(Arc::new)
        .expect("bad certificate or key")
}
