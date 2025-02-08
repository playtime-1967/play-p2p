use libp2p::{multiaddr::Protocol, Multiaddr};
use std::{env, error::Error, fs, path::Path, str::FromStr};

pub fn get_pre_shared_key() -> std::io::Result<Option<String>> {
    let ipfs_path: Box<Path> = env::var("IPFS_PATH")
        .map(|ipfs_path| Path::new(&ipfs_path).into())
        .unwrap_or_else(|_| Path::new("/usr/local/bin/.ipfs").into());

    let swarm_key_file = ipfs_path.join("swarm.key");
    match fs::read_to_string(swarm_key_file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

//parse a legacy multiaddr (replace ipfs with p2p), and strip the peer id so it can be dialed by rust-libp2p
pub fn parse_legacy_multiaddr(text: &str) -> Result<Multiaddr, Box<dyn Error>> {
    let sanitized = text
        .split('/')
        .map(|part| if part == "ipfs" { "p2p" } else { part })
        .collect::<Vec<_>>()
        .join("/");
    let mut res = Multiaddr::from_str(&sanitized)?;
    strip_peer_id(&mut res);
    Ok(res)
}

//modifies a Multiaddr by removing the /p2p/peer_id part. libp2p's dial function does not directly support dialing an address with a peer_id attached.
fn strip_peer_id(addr: &mut Multiaddr) {
    let last = addr.pop();
    println!("last {:?}", last);
    match last {
        Some(Protocol::P2p(peer_id)) => {
            let mut addr = Multiaddr::empty();
            addr.push(Protocol::P2p(peer_id));
            println!("removing peer id {addr} so this address can be dialed by rust-libp2p");
        }

        Some(other) => {
            println!("other {:?}", other);
            addr.push(other)
        }
        _ => {}
    }
}
