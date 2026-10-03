use crate::error::StakingError;
use crate::instructions::shared::mint_rewards;
use crate::state::{Config, StakeAccount, SECONDS_PER_DAY};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    // has_one = owner means only whoever staked can claim
    #[account(
        mut,
        seeds = [b"stake", stake_account.asset.as_ref()],
        bump = stake_account.bump,
        has_one = owner @ StakingError::NotOwner,
    )]
    pub stake_account: Account<'info, StakeAccount>,

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

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimRewards<'info> {
    // the asset isnt even in this instruction, so it stays staked and frozen. nothing here can touch it
    pub fn claim_rewards(&mut self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let days = self.stake_account.full_days(now);
        require!(days > 0, StakingError::NothingToClaim);

        let amount = (days as u64)
            .checked_mul(self.config.points_per_day)
            .ok_or(StakingError::Overflow)?;

        // move the clock forward by whole days only, so a half day isnt thrown away
        self.stake_account.last_claimed += days * SECONDS_PER_DAY;

        mint_rewards(
            &self.config,
            &self.reward_mint.to_account_info(),
            &self.owner_reward_ata.to_account_info(),
            &self.token_program.to_account_info(),
            amount,
        )
    }
}
