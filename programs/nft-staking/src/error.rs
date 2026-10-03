use anchor_lang::prelude::*;

#[error_code]
pub enum StakingError {
    #[msg("you didnt stake this one")]
    NotOwner,
    #[msg("not a full day since the last claim yet")]
    NothingToClaim,
    #[msg("still inside the freeze period, cant unstake yet")]
    FreezePeriodNotOver,
    #[msg("total_staked attribute is missing from the collection")]
    MissingCounter,
    #[msg("total_staked would go below zero")]
    CounterUnderflow,
    #[msg("math overflowed")]
    Overflow,
}
