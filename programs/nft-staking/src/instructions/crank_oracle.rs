use crate::state::{Oracle, CRANK_REWARD, CRANK_WINDOW, MARKET_CLOSE, MARKET_OPEN, SECONDS_PER_DAY};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

// no admin anywhere in here. anyone can crank it
#[derive(Accounts)]
pub struct CrankOracle<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(mut, seeds = [b"oracle"], bump = oracle.bump)]
    pub oracle: Account<'info, Oracle>,

    #[account(mut, seeds = [b"vault"], bump = oracle.vault_bump)]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> CrankOracle<'info> {
    pub fn crank_oracle(&mut self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let new_state = Oracle::transfer_state_at(now);
        let changed = new_state != self.oracle.transfer;
        self.oracle.transfer = new_state;

        // only pay if this crank actually flipped the state and it's right around open or close.
        // otherwise people could spam it at noon and drain the vault
        let secs_today = now.rem_euclid(SECONDS_PER_DAY);
        let near_boundary = (secs_today - MARKET_OPEN).abs() <= CRANK_WINDOW
            || (secs_today - MARKET_CLOSE).abs() <= CRANK_WINDOW;

        // keep the vault rent exempt, an empty-ish vault just stops paying instead of failing
        let rent_floor = Rent::get()?.minimum_balance(0);
        let can_pay = self.vault.lamports() >= CRANK_REWARD + rent_floor;

        if changed && near_boundary && can_pay {
            let seeds: &[&[u8]] = &[b"vault", &[self.oracle.vault_bump]];
            transfer(
                CpiContext::new_with_signer(
                    self.system_program.key(),
                    Transfer {
                        from: self.vault.to_account_info(),
                        to: self.cranker.to_account_info(),
                    },
                    &[seeds],
                ),
                CRANK_REWARD,
            )?;
        }

        Ok(())
    }
}
