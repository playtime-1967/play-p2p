Built a peer-to-peer networking playground with [rust-libp2p](https://github.com/libp2p/rust-libp2p).

**Samples**:   


1- **Chat application**  

A P2P chat application built using libp2p. It enables decentralized communication without a central server. It:   
Uses Gossipsub (a pub-sub protocol) for message broadcasting.  
Uses mDNS (multicast DNS) to discover peers on the local network automatically.  
Supports TCP and QUIC (UDP-based) connections.  
Listens on all available network interfaces and lets the OS choose a port.  
Reads user input from stdin and broadcasts it to connected peers.  


**How to run**:  

Open two or more terminals and run:  
`cargo run --bin chat`  

Wait until you see peer discovery logs indicating that peers have been found.  
Once discovered, the terminals can exchange messages with each other.

Hi!...  
Ola!...

The application handles several types of events, including:   
``` 
Local node is listening on ...  
mDNS discovered a new peer: ...  
Dialing { peer_id:...}  
IncomingConnection { connection_id:... }  
ConnectionClosed { peer_id:...}  
Received message {...}  
ConnectionClosed { peer_id:...} 
```

------------------------------------------------------------------------------------------------------------------------------------------------------------
2- **Distributed key-value store** 

A distributed key-value store using libp2p’s Kademlia DHT. It:  
Allows nodes to store and retrieve values across a peer-to-peer network.  
Listens for new peers and updates routing.  
Responds to key-value queries.  
Finds peers that have stored the key.  


**How to run**:  

Open two or more terminals:  
`cargo run --bin key-val-store`   

``` 
Terminal 1, type PUT my-key my-value  
Terminal 2/3, type GET my-key  
Terminal 2, type PUT_PROVIDER my-key  
Terminal 1/3, type GET_PROVIDERS my-key  
``` 