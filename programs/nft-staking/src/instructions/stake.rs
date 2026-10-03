use crate::instructions::shared::change_total_staked;
use crate::state::{Config, StakeAccount};
use anchor_lang::prelude::*;
use mpl_core::{
    instructions::AddPluginV1CpiBuilder,
    types::{BurnDelegate, FreezeDelegate, Plugin, PluginAuthority},
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = owner,
        seeds = [b"stake", asset.key().as_ref()],
        bump,
        space = 8 + StakeAccount::INIT_SPACE,
    )]
    pub stake_account: Account<'info, StakeAccount>,

    /// CHECK: core checks the owner owns it when we add the owner plugins below
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: core owns it, address pinned to the one in config so you cant stake some random collection
    #[account(mut, address = config.collection)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that signs for the collection and gets the delegates
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

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        // freeze and burn delegates are owner plugins, so the owner signs to add them.
        // the PDA is the authority on both, so later the program can thaw or burn without the owner's key
        AddPluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.owner.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
            .init_authority(PluginAuthority::Address { address: self.update_authority.key() })
            .invoke()?;

        AddPluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.owner.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::BurnDelegate(BurnDelegate {}))
            .init_authority(PluginAuthority::Address { address: self.update_authority.key() })
            .invoke()?;

        self.stake_account.set_inner(StakeAccount {
            owner: self.owner.key(),
            asset: self.asset.key(),
            staked_at: now,
            last_claimed: now,
            bump: bumps.stake_account,
        });

        change_total_staked(
            true,
            &self.collection.to_account_info(),
            &self.update_authority.to_account_info(),
            &self.owner.to_account_info(),
            &self.system_program.to_account_info(),
            &self.core_program.to_account_info(),
            self.config.update_authority_bump,
        )
    }
}
