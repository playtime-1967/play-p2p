use clap::Parser; //Provides a CLI parsing framework. It simplifies handling command-line arguments and flags.
use futures::{prelude::*, StreamExt}; //StreamExt is specifically useful for handling asynchronous streams like network events.
use libp2p::{core::Multiaddr, multiaddr::Protocol};
use std::{error::Error, io::Write, path::PathBuf};
use tokio::task::spawn;
use tracing_subscriber::EnvFilter;
mod file_sharing_network;
use file_sharing_network::network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = ArgDef::parse();

    //The application uses the Client to issue commands (e.g., start_listening, dial).
    //The EventLoop processes these commands and emits events back to the application.
    let (mut network_client, mut network_events, network_event_loop) =
        network::new(args.secret_key_seed).await?;

    //Handling network activities. Spawn the network task for it to run in the background.
    spawn(network_event_loop.run());

    //In case a listen address was provided use it, otherwise listen on any address.
    match args.listen_address {
        Some(addr) => {
            println!("address is provided: {addr}");
            network_client
                .start_listening(addr)
                .await
                .expect("Listening not to fail.")
        }
        None => {
            println!("address is not provided");
            network_client
                .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
                .await
                .expect("Listening not to fail.")
        }
    };

    if let Some(addr) = args.peer {
        if let Some(Protocol::P2p(peer_id)) = addr.iter().last() {
            network_client
                .dial(peer_id, addr)
                .await
                .expect("Dial to succeed");
            println!("Dialed peer: {:?}", peer_id);
        } else {
            return Err("Expect peer multiaddr to contain peer ID.".into());
        }
    }

    match args.action {
        // Providing a file.
        CliAction::Provide { path, name } => {
            // Advertise oneself as a provider of the file on the DHT.
            network_client.start_providing(name.clone()).await;

            loop {
                //A stream of incoming network events (e.g., requests or peer discovery updates)
                match network_events.next().await {
                    // Reply with the content of the file on incoming requests.
                    Some(network::Event::InboundRequest { request, channel }) => {
                        println!("file request {request}");
                        if request == name {
                            //responds via the provided channel.
                            network_client
                                .respond_file(std::fs::read(&path)?, channel)
                                .await;
                        }
                    }
                    e => todo!("{:?}", e), //todo!= panic. conveys an intent of implementing the functionality later
                }
            }
        }
        // Locating and getting a file.
        CliAction::Get { name } => {
            // Locate all nodes providing the file.
            let providers = network_client.get_providers(name.clone()).await;
            println!("Providers for {}: {:?}", name, providers);
            if providers.is_empty() {
                return Err(format!("Could not find provider for file {name}.").into());
            }

            // Request the content of the file from each node.
            let requests = providers.into_iter().map(|p| {
                let mut network_client = network_client.clone();
                let name = name.clone();
                async move { network_client.request_file(p, name).await }.boxed()
            });
            println!("requests {:?}", requests);

            // Await the requests, ignore the remaining once a single one succeeds.
            let file_content = futures::future::select_ok(requests) //select the first successful future over a list of futures.
                .await
                .map_err(|_| "None of the providers returned file.")?
                .0;

            std::io::stdout().write_all(&file_content)?;
        }
    }

    Ok(())
}

//--------------------------------------------- Clap Setup

#[derive(Parser, Debug)]
#[clap(name = "libp2p file sharing example")]
struct ArgDef {
    /// Fixed value to generate deterministic peer ID.
    #[clap(long)]
    secret_key_seed: Option<u8>,

    #[clap(long)]
    peer: Option<Multiaddr>,

    #[clap(long)]
    listen_address: Option<Multiaddr>,

    #[clap(subcommand)]
    action: CliAction,
}

#[derive(Debug, Parser)]
enum CliAction {
    Provide {
        #[clap(long)]
        path: PathBuf,
        #[clap(long)]
        name: String,
    },
    Get {
        #[clap(long)]
        name: String,
    },
}

//How to run: Running three or more nodes
//1- cargo run --bin fish -- --listen-address /ip4/127.0.0.1/tcp/40837 --secret-key-seed 10 provide --path src/main.rs --name main
//2- cargo run --bin fish -- --listen-address /ip4/127.0.0.1/tcp/40839  provide --path plan.md --name plan
//3- cargo run --bin fish -- --peer /ip4/127.0.0.1/tcp/40839/p2p/{peer_id_of_a_provider} get --name plan
