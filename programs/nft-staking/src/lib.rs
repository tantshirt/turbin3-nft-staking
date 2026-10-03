pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("FQPn4yPS42r8feiyP94Gjny7fino6RGikr2pgxfLBPPQ");

#[program]
pub mod nft_staking {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        points_per_day: u64,
        burn_bonus: u64,
        freeze_period: i64,
    ) -> Result<()> {
        ctx.accounts.initialize_config(points_per_day, burn_bonus, freeze_period, &ctx.bumps)
    }

    pub fn init_oracle(ctx: Context<InitOracle>, vault_funding: u64) -> Result<()> {
        ctx.accounts.init_oracle(vault_funding, &ctx.bumps)
    }

    pub fn create_collection(ctx: Context<CreateCollection>, name: String, uri: String) -> Result<()> {
        ctx.accounts.create_collection(name, uri, &ctx.bumps)
    }

    pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
        ctx.accounts.mint_asset(name, uri)
    }

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        ctx.accounts.stake(&ctx.bumps)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        ctx.accounts.claim_rewards()
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        ctx.accounts.unstake()
    }

    pub fn burn_staked_nft(ctx: Context<BurnStakedNft>) -> Result<()> {
        ctx.accounts.burn_staked_nft()
    }

    pub fn crank_oracle(ctx: Context<CrankOracle>) -> Result<()> {
        ctx.accounts.crank_oracle()
    }

    pub fn transfer_asset(ctx: Context<TransferAsset>) -> Result<()> {
        ctx.accounts.transfer_asset()
    }
}
