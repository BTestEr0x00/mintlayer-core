//! Node initialisation routine.

use crate::options::Options;
use common::chain::config::ChainType;
use consensus::rpc::ConsensusRpcServer;
use std::sync::Arc;

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq, Clone, Copy, thiserror::Error)]
enum Error {
    #[error("Chain type '{0}' not yet supported")]
    UnsupportedChain(ChainType),
}

/// Initialize the node, giving caller the opportunity to add more subsystems before start.
pub async fn initialize(opts: Options) -> anyhow::Result<subsystem::Manager> {
    // Initialize storage and chain configuration
    let storage = blockchain_storage::Store::new_empty()?;

    // Chain configuration
    let chain_config = match opts.net {
        ChainType::Mainnet => Arc::new(common::chain::config::create_mainnet()),
        chain_ty => return Err(Error::UnsupportedChain(chain_ty).into()),
    };

    // INITIALIZE SUBSYSTEMS

    let mut manager = subsystem::Manager::new("mintlayer");
    manager.install_signal_handlers();

    // Consensus subsystem
    let consensus = manager.add_subsystem(
        "consensus",
        consensus::make_consensus(chain_config, storage.clone())?,
    );

    // RPC subsystem
    let _rpc = manager.add_subsystem(
        "rpc",
        rpc::Builder::new(opts.rpc_addr)
            .register(consensus.clone().into_rpc())
            .register(NodeRpc::new(manager.make_shutdown_trigger()).into_rpc())
            .build()
            .await?,
    );

    Ok(manager)
}

/// Initialize and run the node
pub async fn run(opts: Options) -> anyhow::Result<()> {
    let manager = initialize(opts).await?;

    #[allow(clippy::unit_arg)]
    Ok(manager.main().await)
}

#[rpc::rpc(server, namespace = "node")]
trait NodeRpc {
    /// Order the node to exit
    #[method(name = "exit")]
    fn exit(&self) -> rpc::Result<()>;

    /// Get node software version
    #[method(name = "version")]
    fn version(&self) -> rpc::Result<String>;
}

struct NodeRpc {
    shutdown_trigger: subsystem::manager::ShutdownTrigger,
}

impl NodeRpc {
    fn new(shutdown_trigger: subsystem::manager::ShutdownTrigger) -> Self {
        Self { shutdown_trigger }
    }
}

impl NodeRpcServer for NodeRpc {
    fn exit(&self) -> rpc::Result<()> {
        self.shutdown_trigger.initiate();
        Ok(())
    }

    fn version(&self) -> rpc::Result<String> {
        Ok(env!("CARGO_PKG_VERSION").into())
    }
}
