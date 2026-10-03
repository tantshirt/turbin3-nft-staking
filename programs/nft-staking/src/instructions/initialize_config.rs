use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        seeds = [b"config"],
        bump,
        space = 8 + Config::INIT_SPACE,
    )]
    pub config: Account<'info, Config>,

    // config is the mint authority so only this program can print rewards
    #[account(
        init,
        payer = admin,
        seeds = [b"rewards", config.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = config,
        mint::token_program = token_program,
    )]
    pub reward_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeConfig<'info> {
    pub fn initialize_config(
        &mut self,
        points_per_day: u64,
        burn_bonus: u64,
        freeze_period: i64,
        bumps: &InitializeConfigBumps,
    ) -> Result<()> {
        // collection gets filled in by create_collection
        self.config.set_inner(Config {
            admin: self.admin.key(),
            collection: Pubkey::default(),
            points_per_day,
            burn_bonus,
            freeze_period,
            update_authority_bump: 0,
            rewards_bump: bumps.reward_mint,
            bump: bumps.config,
        });
        Ok(())
    }
}
