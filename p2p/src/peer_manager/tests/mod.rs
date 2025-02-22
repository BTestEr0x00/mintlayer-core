// Copyright (c) 2022 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://github.com/mintlayer/mintlayer-core/blob/master/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod ban;
mod connections;
mod ping;

use std::{sync::Arc, time::Duration};

use common::time_getter::TimeGetter;
use tokio::{sync::mpsc::UnboundedSender, time::timeout};

use crate::{
    event::PeerManagerEvent,
    interface::types::ConnectedPeer,
    net::{ConnectivityService, NetworkingService},
    peer_manager::PeerManager,
    testing_utils::peerdb_inmemory_store,
    utils::oneshot_nofail,
    P2pConfig,
};

use super::peerdb::storage::PeerDbStorage;

async fn make_peer_manager_custom<T>(
    transport: T::Transport,
    addr: T::Address,
    chain_config: Arc<common::chain::ChainConfig>,
    p2p_config: Arc<P2pConfig>,
    time_getter: TimeGetter,
) -> (
    PeerManager<T, impl PeerDbStorage>,
    UnboundedSender<PeerManagerEvent<T>>,
)
where
    T: NetworkingService + 'static,
    T::ConnectivityHandle: ConnectivityService<T>,
{
    let (conn, _) = T::start(
        transport,
        vec![addr],
        Arc::clone(&chain_config),
        Default::default(),
    )
    .await
    .unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx_sync, mut rx_sync) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move { while rx_sync.recv().await.is_some() {} });

    let peer_manager = PeerManager::<T, _>::new(
        chain_config,
        p2p_config,
        conn,
        rx,
        tx_sync,
        time_getter,
        peerdb_inmemory_store(),
    )
    .unwrap();

    (peer_manager, tx)
}

async fn make_peer_manager<T>(
    transport: T::Transport,
    addr: T::Address,
    chain_config: Arc<common::chain::ChainConfig>,
) -> PeerManager<T, impl PeerDbStorage>
where
    T: NetworkingService + 'static,
    T::ConnectivityHandle: ConnectivityService<T>,
{
    let p2p_config = Arc::new(P2pConfig::default());
    let (peer_manager, _tx) = make_peer_manager_custom::<T>(
        transport,
        addr,
        chain_config,
        p2p_config,
        Default::default(),
    )
    .await;
    peer_manager
}

async fn run_peer_manager<T>(
    transport: T::Transport,
    addr: T::Address,
    chain_config: Arc<common::chain::ChainConfig>,
    p2p_config: Arc<P2pConfig>,
    time_getter: TimeGetter,
) -> UnboundedSender<PeerManagerEvent<T>>
where
    T: NetworkingService + 'static,
    T::ConnectivityHandle: ConnectivityService<T>,
{
    let (mut peer_manager, tx) =
        make_peer_manager_custom::<T>(transport, addr, chain_config, p2p_config, time_getter).await;
    tokio::spawn(async move {
        peer_manager.run().await.unwrap();
    });
    tx
}

async fn get_connected_peers<T: NetworkingService + std::fmt::Debug>(
    tx: &UnboundedSender<PeerManagerEvent<T>>,
) -> Vec<ConnectedPeer> {
    let (rtx, rrx) = oneshot_nofail::channel();
    tx.send(PeerManagerEvent::GetConnectedPeers(rtx)).unwrap();
    timeout(Duration::from_secs(1), rrx).await.unwrap().unwrap()
}
