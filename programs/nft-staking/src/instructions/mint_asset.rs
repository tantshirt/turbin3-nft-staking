use crate::state::Config;
use anchor_lang::prelude::*;
use mpl_core::instructions::CreateV2CpiBuilder;

#[derive(Accounts)]
pub struct MintAsset<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    // brand new keypair, core creates it
    #[account(mut)]
    pub asset: Signer<'info>,

    /// CHECK: core owns it, address pinned to the one in config
    #[account(mut, address = config.collection)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that signs for the collection
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump = config.update_authority_bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    /// CHECK: address pinned to the core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> MintAsset<'info> {
    pub fn mint_asset(&mut self, name: String, uri: String) -> Result<()> {
        let collection_key = self.collection.key();
        let seeds: &[&[u8]] = &[
            b"update_authority",
            collection_key.as_ref(),
            &[self.config.update_authority_bump],
        ];

        // adding an asset to a collection needs the collection's authority to sign, so the PDA does
        CreateV2CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .authority(Some(&self.update_authority.to_account_info()))
            .payer(&self.user.to_account_info())
            .owner(Some(&self.user.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .name(name)
            .uri(uri)
            .invoke_signed(&[seeds])?;

        Ok(())
    }
}
