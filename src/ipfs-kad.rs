use std::{
    num::NonZeroUsize,
    ops::Add,
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use clap::Parser;
use futures::StreamExt;
use libp2p::{
    bytes::BufMut,
    identity, kad, noise,
    swarm::{StreamProtocol, SwarmEvent},
    tcp, yamux, PeerId,
};
use tracing_subscriber::EnvFilter;

const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/ipfs/kad/1.0.0"); //Identifies a protocol for a stream.

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_dns()? //used for resolving domain names (dnsaddr) into IP addresses. In this example, DNS helps resolve boot node addresses like bootstrap.libp2p.io into their respective IP addresses.
        .with_behaviour(|key| {
            // Create a Kademlia behaviour.
            let mut kad_cfg = kad::Config::new(IPFS_PROTO_NAME);
            kad_cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let store = kad::store::MemoryStore::new(key.public().to_peer_id());
            kad::Behaviour::with_config(key.public().to_peer_id(), store, kad_cfg)
        })?
        .build();

    // Add the bootnodes to the local routing table.
    //`libp2p-dns` built into the `transport` resolves the `dnsaddr` when Kademlia tries to dial these nodes.
    for peer in &BOOTNODES {
        swarm
            .behaviour_mut()
            //a multiaddr that uses DNS to resolve the actual IP addresses of the bootstrap nodes... "/dnsaddr/bootstrap.libp2p.io"
            .add_address(&peer.parse()?, "/dnsaddr/bootstrap.libp2p.io".parse()?);
    }

    let cli_action = CliAction::parse();

    match cli_action.argument {
        CliArgument::GetPeers { peer_id } => {
            let peer_id = peer_id.unwrap_or(PeerId::random());
            println!("Searching for the closest peers to {peer_id}");
            //get_closest_peers modifies the internal state of the Behaviour (by starting a query), so it requires a mutable reference
            swarm.behaviour_mut().get_closest_peers(peer_id);
        }
        CliArgument::PutPkRecord {} => {
            println!("Putting PK record into the DHT");

            let mut pk_record_key = vec![];
            pk_record_key.put_slice("/pk/".as_bytes()); //Transfer bytes into self from src and advance the cursor by the number of bytes written.
            pk_record_key.put_slice(swarm.local_peer_id().to_bytes().as_slice());

            let mut pk_record =
                kad::Record::new(pk_record_key, local_key.public().encode_protobuf());
            pk_record.publisher = Some(*swarm.local_peer_id());
            pk_record.expires = Some(Instant::now().add(Duration::from_secs(60)));

            swarm
                .behaviour_mut()
                //Quorum specifies the number of peers that must successfully store a record for the operation to be considered successful.
                .put_record(pk_record, kad::Quorum::N(NonZeroUsize::new(3).unwrap()))?;
        }
    }

    loop {
        let event = swarm.select_next_some().await;

        match event {
            //OutboundQueryProgressed is an event generated when an outbound query (e.g., get_closest_peers, put_record) makes progress.
            //It provides updates on the query, including whether it succeeded, failed, or timed out.
            SwarmEvent::Behaviour(kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::GetClosestPeers(Ok(get_closest_peers_result)),
                .. // ignoring the rest of the fields. You only care about get_closest_peers_result
            }) => {
                // The example is considered failed as there should always be at least 1 reachable peer.
                if get_closest_peers_result.peers.is_empty() {
                    bail!("Query finished with no closest peers.")
                }

                println!(
                    "Query finished with closest peers: {:#?}",
                    get_closest_peers_result.peers
                );

                return Ok(());
            }
            SwarmEvent::Behaviour(kad::Event::OutboundQueryProgressed {
                result:
                    kad::QueryResult::GetClosestPeers(Err(kad::GetClosestPeersError::Timeout {
                        ..
                    })),
                ..
            }) => {
                bail!("Query for closest peers timed out")
            }
            SwarmEvent::Behaviour(kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::PutRecord(Ok(_)),
                ..
            }) => {
                println!("Successfully inserted the PK record");

                return Ok(());
            }
            SwarmEvent::Behaviour(kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::PutRecord(Err(err)),
                ..
            }) => {
                bail!(anyhow::Error::new(err).context("Failed to insert the PK record"));
            }
            _ => {}
        }
    }
}

//-------------------------------------Clap Setup
#[derive(Parser, Debug)]
#[clap(name = "libp2p Kademlia DHT example")]
struct CliAction {
    #[clap(subcommand)]
    argument: CliArgument,
}

#[derive(Parser, Debug)]
enum CliArgument {
    GetPeers {
        #[clap(long)]
        peer_id: Option<PeerId>,
    },
    PutPkRecord {},
}

//How to run: Running a node- two scenarios
//1- get-peers
//cargo run --bin ipfs -- get-peers --peer-id {peer_id}
//or
//cargo run --bin ipfs -- get-peers
// 2- put record
// cargo run --bin ipfs -- put-pk-record
