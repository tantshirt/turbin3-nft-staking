use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub collection: Pubkey,
    pub points_per_day: u64,
    pub burn_bonus: u64,
    pub freeze_period: i64,
    pub update_authority_bump: u8,
    pub rewards_bump: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct StakeAccount {
    pub owner: Pubkey,
    pub asset: Pubkey,
    pub staked_at: i64,
    pub last_claimed: i64,
    pub bump: u8,
}

pub const SECONDS_PER_DAY: i64 = 86_400;

impl StakeAccount {
    // only whole days count. leftover seconds roll into the next claim
    pub fn full_days(&self, now: i64) -> i64 {
        (now - self.last_claimed) / SECONDS_PER_DAY
    }
}

// same order as core's ExternalValidationResult, the byte values have to match
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum ValidationResult {
    Approved,
    Rejected,
    Pass,
}

// core reads this account raw. after anchor's 8 byte discriminator it expects
// OracleValidation::V1 { create, transfer, burn, update }, so the first byte is the
// enum tag (1 = V1) and then four results. bumps go after, core doesnt read past that
#[account]
#[derive(InitSpace)]
pub struct Oracle {
    pub version: u8,
    pub create: ValidationResult,
    pub transfer: ValidationResult,
    pub burn: ValidationResult,
    pub update: ValidationResult,
    pub bump: u8,
    pub vault_bump: u8,
}

pub const MARKET_OPEN: i64 = 9 * 3600;
pub const MARKET_CLOSE: i64 = 17 * 3600;
// cranking within 5 minutes of open or close gets paid
pub const CRANK_WINDOW: i64 = 300;
pub const CRANK_REWARD: u64 = 1_000_000;

impl Oracle {
    // 9:00:00 is open, 17:00:00 is already closed
    pub fn transfer_state_at(now: i64) -> ValidationResult {
        let secs_today = now.rem_euclid(SECONDS_PER_DAY);
        if secs_today >= MARKET_OPEN && secs_today < MARKET_CLOSE {
            ValidationResult::Pass
        } else {
            ValidationResult::Rejected
        }
    }
}
