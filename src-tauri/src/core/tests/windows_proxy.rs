//! FEAT-18 experiment. A test-only HTTP/1.1 proxy in the Handoff process,
//! with exactly two loopback upstreams. No machine trust or installed proxy.
use super::*;
use crate::run::tests::{finished, start_with_env};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

const HOST: &str = "api.stripe.test";
const KEY: &[u8] = b"sk_test_51SyntheticProxyExperiment0123456789XYZ";

fn headers(stream: impl std::io::Read) -> Vec<String> {
    let mut reader = BufReader::new(stream);
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        if line == "\r\n" {
            return lines;
        }
        lines.push(line.trim_end().to_owned());
    }
}

#[test]
#[ignore = "loopback proxy experiment; requires Python, Node, sh and openssl on PATH"]
fn windows_python_uses_a_standin_through_a_verifying_local_proxy() {
    let me = app("Proxy");
    let dir = &me.data;
    let servers = windows_tls_fixture(dir).expect("the Windows TLS fixture requires Node, sh and openssl");
    let good = servers.ports[HOST].as_u64().unwrap() as u16;
    let wrong_cert = servers.ports["evil.test"].as_u64().unwrap() as u16;
    let ca = dir.join("ca.pem");
    let h = me.handoff.clone();
    {
        let mut core = h.lock();
        let key = core.vault.add("Stripe", None, None, &Secret::new(KEY.to_vec()), Keep::Forever, now());
        core.vault.set_hosts(key, vec![HOST.into()], Some(ca.clone()));
    }
    let profile = format!("test-{}", OsRng.next_u32());
    let served = h.clone();
    os::listen_for_keys(&profile, Arc::new(move |peer, stream| crate::pointer::serve(&served, &peer, stream))).unwrap();
    fs::write(dir.join(".env"), "STRIPE_API_KEY=handoff:Stripe\n").unwrap();

    // Use the fixture cert only in this test's downstream TLS endpoint.
    let cert = CertificateDer::from_pem_file(dir.join(format!("{HOST}.pem"))).unwrap();
    let key = PrivateKeyDer::from_pem_file(dir.join(format!("{HOST}.key"))).unwrap();
    let downstream = Arc::new(ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert], key).unwrap());
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from_pem_file(&ca).unwrap()).unwrap();
    let upstream = Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth());
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let proxy_port = listener.local_addr().unwrap().port().to_string();
    let proxy = std::thread::spawn(move || {
        let mut outcomes = Vec::new();
        let mut standin = String::new();
        for _ in 0..3 {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let connect = headers(&mut socket);
            let mut request = connect[0].split_ascii_whitespace();
            assert_eq!(request.next(), Some("CONNECT"));
            let (host, port) = request.next().unwrap().split_once(':').unwrap();
            let port: u16 = port.parse().unwrap();
            if host != HOST {
                socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").unwrap();
                outcomes.push("outside scope");
                continue;
            }
            assert!(port == good || port == wrong_cert);
            socket.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").unwrap();
            let mut client = StreamOwned::new(ServerConnection::new(downstream.clone()).unwrap(), socket);
            let request = headers(&mut client);
            assert_eq!(request[0], "GET /probe HTTP/1.1");
            let bearer = request.iter().filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization")).unwrap().1.trim().strip_prefix("Bearer ").unwrap();

            let socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut origin = StreamOwned::new(ClientConnection::new(upstream.clone(), HOST.try_into().unwrap()).unwrap(), socket);
            if let Err(error) = origin.conn.complete_io(&mut origin.sock) {
                assert!(matches!(error.get_ref().and_then(|e| e.downcast_ref::<rustls::Error>()), Some(rustls::Error::InvalidCertificate(_))), "{error}");
                client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n").unwrap();
                outcomes.push("invalid certificate");
                continue;
            }
            // Same live RunSwap produced by the runner's approved pipe request.
            // No swap token or raw-value reply is sent to the Python process.
            {
                let core = h.lock();
                assert_eq!(core.runs.len(), 1);
                let swap = core.runs[0].swaps.iter().find(|s| s.placeholder.as_str() == bearer && s.hosts.iter().any(|h| h == host)).unwrap();
                assert_ne!(swap.secret.expose(), bearer.as_bytes());
                standin = bearer.to_owned();
                write!(origin, "GET /probe HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAuthorization: Bearer ").unwrap();
                origin.write_all(swap.secret.expose()).unwrap();
                origin.write_all(b"\r\n\r\n").unwrap();
                origin.flush().unwrap();
            }
            assert!(headers(&mut origin)[0].contains(" 200 "));
            client.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").unwrap();
            outcomes.push("forwarded");
        }
        (standin, outcomes)
    });
    // Ordinary urllib reads its proxy and CA from this child process's environment.
    fs::write(dir.join("client.py"), r#"import hashlib, os, sys, urllib.error, urllib.request
key = os.environ["STRIPE_API_KEY"]
print("initial", hashlib.sha256(key.encode()).hexdigest())
assert "KEYHANDOFF_HOOK" not in os.environ
for host, port, expected in [("api.stripe.test", sys.argv[1], 200), ("api.stripe.test", sys.argv[2], 502), ("evil.test", sys.argv[2], 403)]:
    req = urllib.request.Request(f"https://{host}:{port}/probe", headers={"Authorization": "Bearer " + key})
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            assert expected == r.status == 200 and r.read() == b"ok"
    except urllib.error.HTTPError as e:
        assert expected == e.code == 502
    except urllib.error.URLError as e:
        assert expected == 403 and "403" in str(e.reason)
"#).unwrap();
    let out = start_with_env(dir, &profile, &["python", "client.py", &good.to_string(), &wrong_cert.to_string()], None,
        &[("KEYHANDOFF_TEST_PROXY_PORT", &proxy_port), ("SSL_CERT_FILE", ca.to_str().unwrap())]);
    answer_when_asked(&me.handoff, Answer::Once);
    let (log, code) = finished(&out);
    assert_eq!(code, 0, "{log}");
    let (standin, outcomes) = proxy.join().unwrap();
    assert_eq!(outcomes, ["forwarded", "invalid certificate", "outside scope"]);
    assert!(log.contains(&format!("initial {:x}", Sha256::digest(standin.as_bytes()))));
    drop(servers);
    let received = fs::read_to_string(dir.join(format!("{HOST}.jsonl"))).unwrap();
    assert_eq!(received.lines().count(), 1);
    assert!(received.contains(&format!("Bearer {}", std::str::from_utf8(KEY).unwrap())));
    let refused = dir.join("evil.test.jsonl");
    assert!(!refused.exists() || fs::read(&refused).unwrap().is_empty());
    fs::remove_dir_all(dir).unwrap();
}
