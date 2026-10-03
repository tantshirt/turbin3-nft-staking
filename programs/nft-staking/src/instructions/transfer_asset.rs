use crate::state::{Config, Oracle};
use anchor_lang::prelude::*;
use mpl_core::instructions::TransferV1CpiBuilder;

#[derive(Accounts)]
pub struct TransferAsset<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: whoever's getting the NFT, they dont need to sign
    pub new_owner: UncheckedAccount<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// CHECK: core checks the owner signed for it
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: core owns it, address pinned to the one in config
    #[account(mut, address = config.collection)]
    pub collection: UncheckedAccount<'info>,

    #[account(seeds = [b"oracle"], bump = oracle.bump)]
    pub oracle: Account<'info, Oracle>,

    /// CHECK: address pinned to the core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> TransferAsset<'info> {
    pub fn transfer_asset(&mut self) -> Result<()> {
        // core has to see the oracle account to run the transfer check, so it rides along as a remaining account
        TransferV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.owner.to_account_info()))
            .new_owner(&self.new_owner.to_account_info())
            .system_program(Some(&self.system_program.to_account_info()))
            .add_remaining_account(&self.oracle.to_account_info(), false, false)
            .invoke()?;

        Ok(())
    }
}
