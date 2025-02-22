// Copyright (c) 2023 RBB S.r.l
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

use std::sync::Arc;

use common::chain::config;

use crate::{
    config::P2pConfig,
    event::PeerManagerEvent,
    message::{PeerManagerResponse, PingRequest, PingResponse, Request},
    net::{
        default_backend::{
            transport::TcpTransportSocket,
            types::{Command, ConnectivityEvent, PeerId},
            ConnectivityHandle, DefaultNetworkingService,
        },
        types::PeerInfo,
    },
    peer_manager::PeerManager,
    testing_utils::{peerdb_inmemory_store, P2pTestTimeGetter},
};

#[tokio::test]
async fn ping_timeout() {
    type TestNetworkingService = DefaultNetworkingService<TcpTransportSocket>;

    let chain_config = Arc::new(config::create_mainnet());
    let p2p_config: Arc<P2pConfig> = Arc::new(Default::default());
    let ping_check_period = *p2p_config.ping_check_period;
    let ping_timeout = *p2p_config.ping_timeout;

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (conn_tx, conn_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_peer_tx, peer_rx) =
        tokio::sync::mpsc::unbounded_channel::<PeerManagerEvent<TestNetworkingService>>();
    let time_getter = P2pTestTimeGetter::new();
    let (sync_tx, _sync_rx) = tokio::sync::mpsc::unbounded_channel();
    let connectivity_handle = ConnectivityHandle::<TestNetworkingService, TcpTransportSocket>::new(
        vec![],
        cmd_tx,
        conn_rx,
    );

    let mut peer_manager = PeerManager::new(
        Arc::clone(&chain_config),
        p2p_config,
        connectivity_handle,
        peer_rx,
        sync_tx,
        time_getter.get_time_getter(),
        peerdb_inmemory_store(),
    )
    .unwrap();

    tokio::spawn(async move {
        let _ = peer_manager.run().await;
    });

    // Notify about new inbound connection
    conn_tx
        .send(ConnectivityEvent::InboundAccepted {
            address: "123.123.123.123:12345".parse().unwrap(),
            peer_info: PeerInfo {
                peer_id: PeerId::new(),
                network: *chain_config.magic_bytes(),
                version: *chain_config.version(),
                agent: None,
                subscriptions: Default::default(),
            },
            receiver_address: None,
        })
        .unwrap();

    // Receive ping requests and send responses normally
    for _ in 0..30 {
        time_getter.advance_time(ping_check_period).await;

        let event = cmd_rx.recv().await.unwrap();
        match event {
            Command::SendRequest {
                peer_id,
                request_id,
                message: Request::PingRequest(PingRequest { nonce }),
            } => {
                conn_tx
                    .send(ConnectivityEvent::Response {
                        peer_id,
                        request_id,
                        response: PeerManagerResponse::PingResponse(PingResponse { nonce }),
                    })
                    .unwrap();
            }
            _ => panic!("unexpected event: {event:?}"),
        }
    }

    // Receive one more ping request but do not send a ping response
    time_getter.advance_time(ping_check_period).await;
    let event = cmd_rx.recv().await.unwrap();
    match event {
        Command::SendRequest {
            peer_id: _,
            request_id: _,
            message: Request::PingRequest(PingRequest { nonce: _ }),
        } => {}
        _ => panic!("unexpected event: {event:?}"),
    }

    time_getter.advance_time(ping_timeout).await;

    // PeerManager should ask backend to close connection
    let event = cmd_rx.recv().await.unwrap();
    match event {
        Command::Disconnect { peer_id } => {
            conn_tx.send(ConnectivityEvent::ConnectionClosed { peer_id }).unwrap();
        }
        _ => panic!("unexpected event: {event:?}"),
    }
}
