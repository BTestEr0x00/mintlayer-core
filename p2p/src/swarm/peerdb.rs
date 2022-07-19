// Copyright (c) 2022 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://spdx.org/licenses/MIT
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Author(s): A. Altonen

//! Peer database
//!
//! The peer database recognizes three peer types:
//! - active peers
//! - idle peers
//! - reserved peers (not implemented)
//!
//! Active peers are those peers that the [`crate::swarm::PeerManager`] has an active connection with
//! whereas idle peers are the peers that are known to `PeerDb` but are not part of our swarm.
//! Idle peers are discovered through various peer discovery mechanisms and they are used by
//! [`crate::swarm::PeerManager::heartbeat()`] to establish new outbound connections if the actual
//! number of active connectios is less than the desired number of connections.
//!
//! TODO: reserved peers

use crate::net::{types, NetworkingService};
use logging::log;
use std::collections::{hash_map::Entry, HashMap, HashSet};

// TODO: store active address
// TODO: store other discovered addresses
#[derive(Debug)]
pub struct PeerContext<T: NetworkingService> {
    /// Peer information
    pub _info: types::PeerInfo<T>,

    /// Peer score
    pub score: u32,
}

/// Peer address information
struct PeerAddrInfo<T: NetworkingService> {
    /// Hashset of IPv4 addresses
    ip4: HashSet<T::Address>,

    /// Hashset of IPv6 addresses
    ip6: HashSet<T::Address>,
}

enum Peer<T: NetworkingService> {
    /// Known peer (`types::PeerInfo<t>` received)
    _Known(PeerContext<T>),

    /// Discovered peer (`types::PeerAddrInfo<T>` received)
    Discovered(PeerAddrInfo<T>),
}

// TODO: improve how peers are stored, order by reputation?
pub struct PeerDb<T: NetworkingService> {
    /// Set of peers known to `PeerDb`
    peers: HashMap<T::PeerId, Peer<T>>,

    /// Set of available (idle) peers
    available: HashSet<T::PeerId>,

    /// Pending connections
    pending: HashMap<T::Address, T::PeerId>,

    /// Banned peers
    banned: HashSet<T::PeerId>,
}

impl<T: NetworkingService> Default for PeerDb<T> {
    fn default() -> Self {
        Self {
            peers: Default::default(),
            available: Default::default(),
            pending: Default::default(),
            banned: Default::default(),
        }
    }
}

impl<T: NetworkingService> PeerDb<T> {
    pub fn new() -> Self {
        Default::default()
    }

    /// Get the number of idle (available) peers
    pub fn idle_peer_count(&self) -> usize {
        self.available.len()
    }

    /// Get socket address of the next best peer (TODO: in terms of peer score)
    // TODO: rewrite all of this
    pub fn best_peer_addr(&mut self) -> Option<T::Address> {
        // TODO: improve peer selection
        let peer_id = match self.available.iter().next() {
            Some(peer_id) => *peer_id,
            None => return None,
        };

        let addr = match self.peers.get(&peer_id) {
            Some(Peer::_Known(_)) => {
                unimplemented!()
            }
            Some(Peer::Discovered(peer_info)) => {
                // TODO: rewrite this, if there are no addressess, the peer is forgotten
                assert!(!(peer_info.ip4.is_empty() && peer_info.ip6.is_empty()));

                if peer_info.ip6.is_empty() {
                    peer_info.ip4.iter().next().expect("peer_info.ip4 empty").clone()
                } else {
                    peer_info.ip6.iter().next().expect("peer_info.ip6 empty").clone()
                }
            }
            None => {
                log::error!("PeerDb has an inconsistent state");
                return None;
            }
        };

        self.pending.insert(addr.clone(), peer_id);
        Some(addr)
    }

    /// Check is the peer ID banned
    pub fn is_id_banned(&self, peer_id: &T::PeerId) -> bool {
        self.banned.contains(peer_id)
    }

    /// Check is the address banned
    pub fn is_address_banned(&self, _address: &T::Address) -> bool {
        false // TODO: implement
    }

    /// Discover new peer addresses
    pub fn discover_peers(&mut self, peers: &[types::AddrInfo<T>]) {
        for info in peers.iter() {
            match self.peers.entry(info.peer_id) {
                Entry::Occupied(mut entry) => match entry.get_mut() {
                    Peer::Discovered(addr_info) => {
                        addr_info.ip4.extend(info.ip4.clone());
                        addr_info.ip6.extend(info.ip6.clone());
                    }
                    Peer::_Known(_info) => {
                        // TODO: update existing information of a known peer
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(Peer::Discovered(PeerAddrInfo {
                        ip4: HashSet::from_iter(info.ip4.iter().cloned()),
                        ip6: HashSet::from_iter(info.ip6.iter().cloned()),
                    }));
                    self.available.insert(info.peer_id);
                }
            }
        }
    }

    /// Expire discovered peer addresses
    pub fn expire_peers(&mut self, _peers: &[types::AddrInfo<T>]) {
        // TODO: implement
    }

    /// Report outbound connection failure
    ///
    /// When [`crate::swarm::PeerManager::heartbeat()`] has initiated an outbound connection
    /// and the connection is refused, it's reported back to the `PeerDb` so it knows to update
    /// the peer information accordingly by forgetting the address and adjusting the peer score
    /// appropriately.
    pub fn report_outbound_failure(&mut self, _address: T::Address) {
        // TODO: implement
    }

    /// Register peer information to `PeerDb`
    pub fn register_peer_info(&mut self, _info: types::PeerInfo<T>) {
        // TODO: implement
    }

    /// Ban peer
    pub fn ban_peer(&mut self, peer_id: &T::PeerId) {
        // TODO: print more information about the peer
        self.banned.insert(*peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{self, libp2p::Libp2pService};
    use libp2p::PeerId;

    #[test]
    fn test_peer_discovered_libp2p() {
        let mut peerdb = PeerDb::new();

        let id_1: libp2p::PeerId = PeerId::random();
        let id_2: libp2p::PeerId = PeerId::random();
        let id_3: libp2p::PeerId = PeerId::random();

        // check that peer with `id` has the correct ipv4 and ipv6 addresses
        let check_peer =
            |peers: &HashMap<<Libp2pService as NetworkingService>::PeerId, Peer<Libp2pService>>,
             peer_id: libp2p::PeerId,
             ip4: Vec<<Libp2pService as NetworkingService>::Address>,
             ip6: Vec<<Libp2pService as NetworkingService>::Address>| {
                let (p_ip4, p_ip6) = {
                    match peers.get(&peer_id).unwrap() {
                        Peer::_Known(_) => panic!("invalid peer type"),
                        Peer::Discovered(info) => (info.ip4.clone(), info.ip6.clone()),
                    }
                };

                assert_eq!(ip4.len(), p_ip4.len());
                assert_eq!(ip6.len(), p_ip6.len());

                for ip in ip4.iter() {
                    assert!(p_ip4.contains(ip));
                }

                for ip in ip6.iter() {
                    assert!(p_ip6.contains(ip));
                }
            };

        // first add two new peers, both with ipv4 and ipv6 address
        peerdb.discover_peers(&[
            net::types::AddrInfo {
                peer_id: id_1,
                ip4: vec!["/ip4/127.0.0.1/tcp/9090".parse().unwrap()],
                ip6: vec!["/ip6/::1/tcp/9091".parse().unwrap()],
            },
            net::types::AddrInfo {
                peer_id: id_2,
                ip4: vec!["/ip4/127.0.0.1/tcp/9092".parse().unwrap()],
                ip6: vec!["/ip6/::1/tcp/9093".parse().unwrap()],
            },
        ]);

        assert_eq!(peerdb.peers.len(), 2);
        assert_eq!(
            peerdb.peers.iter().filter(|x| std::matches!(x.1, Peer::_Known(_))).count(),
            0
        );
        assert_eq!(peerdb.available.len(), 2);

        check_peer(
            &peerdb.peers,
            id_1,
            vec!["/ip4/127.0.0.1/tcp/9090".parse().unwrap()],
            vec!["/ip6/::1/tcp/9091".parse().unwrap()],
        );

        check_peer(
            &peerdb.peers,
            id_2,
            vec!["/ip4/127.0.0.1/tcp/9092".parse().unwrap()],
            vec!["/ip6/::1/tcp/9093".parse().unwrap()],
        );

        // then discover one new peer and two additional ipv6 addresses for peer 1
        peerdb.discover_peers(&[
            net::types::AddrInfo {
                peer_id: id_1,
                ip4: vec![],
                ip6: vec![
                    "/ip6/::1/tcp/9094".parse().unwrap(),
                    "/ip6/::1/tcp/9095".parse().unwrap(),
                ],
            },
            net::types::AddrInfo {
                peer_id: id_3,
                ip4: vec!["/ip4/127.0.0.1/tcp/9096".parse().unwrap()],
                ip6: vec!["/ip6/::1/tcp/9097".parse().unwrap()],
            },
        ]);

        check_peer(
            &peerdb.peers,
            id_1,
            vec!["/ip4/127.0.0.1/tcp/9090".parse().unwrap()],
            vec![
                "/ip6/::1/tcp/9091".parse().unwrap(),
                "/ip6/::1/tcp/9094".parse().unwrap(),
                "/ip6/::1/tcp/9095".parse().unwrap(),
            ],
        );

        check_peer(
            &peerdb.peers,
            id_3,
            vec!["/ip4/127.0.0.1/tcp/9096".parse().unwrap()],
            vec!["/ip6/::1/tcp/9097".parse().unwrap()],
        );
    }
}
