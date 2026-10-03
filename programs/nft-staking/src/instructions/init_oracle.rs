use crate::state::{Config, Oracle, ValidationResult};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

#[derive(Accounts)]
pub struct InitOracle<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    // static address so the collection plugin can point at it forever
    #[account(
        init,
        payer = admin,
        seeds = [b"oracle"],
        bump,
        space = 8 + Oracle::INIT_SPACE,
    )]
    pub oracle: Account<'info, Oracle>,

    // plain lamports pot that pays whoever cranks the oracle on time
    #[account(mut, seeds = [b"vault"], bump)]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitOracle<'info> {
    pub fn init_oracle(&mut self, vault_funding: u64, bumps: &InitOracleBumps) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        // only transfer is ever checked, the rest just say pass
        self.oracle.set_inner(Oracle {
            version: 1,
            create: ValidationResult::Pass,
            transfer: Oracle::transfer_state_at(now),
            burn: ValidationResult::Pass,
            update: ValidationResult::Pass,
            bump: bumps.oracle,
            vault_bump: bumps.vault,
        });

        transfer(
            CpiContext::new(
                self.system_program.key(),
                Transfer {
                    from: self.admin.to_account_info(),
                    to: self.vault.to_account_info(),
                },
            ),
            vault_funding,
        )
    }
}
