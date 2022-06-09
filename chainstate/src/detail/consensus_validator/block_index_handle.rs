use common::{
    chain::block::{Block, BlockIndex},
    primitives::{BlockHeight, Id},
};

use crate::detail::PropertyQueryError;

pub trait BlockIndexHandle {
    fn get_block_index(
        &self,
        block_index: &Id<Block>,
    ) -> Result<Option<BlockIndex>, PropertyQueryError>;
    fn get_ancestor(
        &self,
        block_index: &BlockIndex,
        ancestor_height: BlockHeight,
    ) -> Result<BlockIndex, PropertyQueryError>;
}
