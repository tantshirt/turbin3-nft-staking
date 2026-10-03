use crate::error::StakingError;
use crate::instructions::shared::{change_total_staked, mint_rewards};
use crate::state::{Config, StakeAccount};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_core::{
    instructions::{BurnV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{FreezeDelegate, Plugin},
};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        close = owner,
        seeds = [b"stake", asset.key().as_ref()],
        bump = stake_account.bump,
        has_one = owner @ StakingError::NotOwner,
        has_one = asset,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    /// CHECK: pinned by has_one on the stake account
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: core owns it, address pinned to the one in config
    #[account(mut, address = config.collection)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that signs for the collection and holds the burn delegate
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump = config.update_authority_bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"rewards", config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = reward_mint,
        associated_token::authority = owner,
        associated_token::token_program = token_program,
    )]
    pub owner_reward_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: address pinned to the core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> BurnStakedNft<'info> {
    pub fn burn_staked_nft(&mut self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        // no freeze period check here. you're giving the NFT up forever, that's the trade
        let days = self.stake_account.full_days(now) as u64;
        let amount = days
            .checked_mul(self.config.points_per_day)
            .and_then(|r| r.checked_add(self.config.burn_bonus))
            .ok_or(StakingError::Overflow)?;
        mint_rewards(
            &self.config,
            &self.reward_mint.to_account_info(),
            &self.owner_reward_ata.to_account_info(),
            &self.token_program.to_account_info(),
            amount,
        )?;

        let collection_key = self.collection.key();
        let seeds: &[&[u8]] = &[
            b"update_authority",
            collection_key.as_ref(),
            &[self.config.update_authority_bump],
        ];

        // a frozen asset cant be burned, even by the burn delegate, so thaw it first
        UpdatePluginV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.update_authority.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(&[seeds])?;

        // the PDA burns it as the BurnDelegate, the owner's signature isnt what core checks here
        BurnV1CpiBuilder::new(&self.core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.update_authority.to_account_info()))
            .system_program(Some(&self.system_program.to_account_info()))
            .invoke_signed(&[seeds])?;

        change_total_staked(
            false,
            &self.collection.to_account_info(),
            &self.update_authority.to_account_info(),
            &self.owner.to_account_info(),
            &self.system_program.to_account_info(),
            &self.core_program.to_account_info(),
            self.config.update_authority_bump,
        )
    }
}
