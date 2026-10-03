# NFT Staking on Metaplex Core

Week 5 for Turbin3. Stake a Core NFT, earn reward tokens every day it sits there, claim
them whenever, or burn the NFT for a big one-time bonus. The collection keeps its own
count of how many are staked, and an oracle only lets NFTs change hands 9 to 5 UTC.

Wrote it from scratch instead of building on the class program.

## Running it

```bash
pnpm install
anchor build
anchor test --validator legacy
```

You need `--validator legacy`. Anchor 1.0 goes for Surfpool by default and I don't have
it installed.

19 tests pass. Screenshot is in `screenshots/`.

The tests run inside LiteSVM, not the validator. Rewards count whole days and the oracle
cares what hour it is, so on a real validator the results would depend on when you hit
enter. LiteSVM lets me set the clock to the exact second.

Metaplex Core isn't on a fresh local chain, so `tests/fixtures/mpl_core.so` is the mainnet
program, dumped with:

```bash
solana program dump -u m CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d tests/fixtures/mpl_core.so
```

---

## How it works

Core NFTs are one account. No mint, no token account, no metadata PDA. Extra behavior
comes from plugins stuck onto the asset or the collection, and that's what this whole
program is. It mostly just adds and removes plugins through CPI.

```rust
pub struct Config {
    pub admin: Pubkey,
    pub collection: Pubkey,
    pub points_per_day: u64,     // reward tokens per full day staked
    pub burn_bonus: u64,         // one time payout for burning
    pub freeze_period: i64,      // seconds before you're allowed to unstake
    pub update_authority_bump: u8,
    pub rewards_bump: u8,
    pub bump: u8,
}

pub struct StakeAccount {
    pub owner: Pubkey,
    pub asset: Pubkey,
    pub staked_at: i64,
    pub last_claimed: i64,
    pub bump: u8,
}
```

| Account | Seeds | What it's for |
|---|---|---|
| Config | `["config"]` | settings, and the reward mint authority |
| Reward mint | `["rewards", config]` | the token you earn |
| Update authority | `["update_authority", collection]` | owns the collection, and is the freeze + burn delegate on every staked NFT |
| Stake account | `["stake", asset]` | who staked it and when |
| Oracle | `["oracle"]` | the Approved / Rejected / Pass state Core reads on transfer |
| Vault | `["vault"]` | lamports that pay people for cranking the oracle |

Instructions:

- `initialize_config` sets the numbers and makes the reward mint
- `init_oracle` makes the oracle and funds the vault
- `create_collection` makes the collection with the `total_staked` attribute and the oracle plugin already on it
- `mint_asset` mints an NFT into the collection
- `stake` / `unstake`
- `claim_rewards` and `burn_staked_nft`, tasks 1.1 and 1.2
- `crank_oracle` and `transfer_asset`, task 2

### Staking

`stake` adds two plugins to the NFT. `FreezeDelegate { frozen: true }` so it can't move,
and `BurnDelegate` so the program can burn it later. Both are owner plugins, so the owner
has to sign to add them. Both get the update authority PDA as their authority. After
that the program can thaw or burn the NFT without ever needing the owner's key again.

`unstake` checks the freeze period, pays out any full days left, thaws, removes both
plugins, and closes the stake account.

### Claim without unstaking

Task 1.1. `claim_rewards` doesn't even take the asset as an account. It reads the stake
account, mints `full_days * points_per_day` to your ATA, and moves `last_claimed` forward.
Nothing in that instruction can touch the NFT, so it stays frozen and staked by default.

`last_claimed` goes forward by whole days, not to `now`. Claim at 1.5 days and you get one
day's worth, and the half day is still there next time.

### Burn to earn

Task 1.2. `burn_staked_nft` mints the pending rewards plus `burn_bonus`, then burns the
NFT with `BurnV1` signed by the update authority PDA. That only works because the PDA is
the BurnDelegate. No freeze period check, you're giving it up for good.

### total_staked on the collection

Task 1.3. The counter is an `Attributes` plugin on the collection itself, not on any one
NFT. Attributes are strings, so every stake reads `"3"`, parses it, writes `"4"` back with
`UpdateCollectionPluginV1`. Stake adds one, unstake and burn take one off. Underflow is a
`require!` instead of a wrap.

### The oracle

Task 2. The collection has an Oracle plugin pointing at the `["oracle"]` PDA, checking
only `Transfer`, with only reject power. Every time an NFT in the collection transfers,
Core reads the oracle account and if it says `Rejected` the transfer dies.

Core reads that account raw, so the layout matters:

```rust
pub struct Oracle {
    pub version: u8,                 // always 1, core's OracleValidation::V1 tag
    pub create: ValidationResult,
    pub transfer: ValidationResult,  // the only one that ever changes
    pub burn: ValidationResult,
    pub update: ValidationResult,
    pub bump: u8,
    pub vault_bump: u8,
}
```

`ValidationResultsOffset::Anchor` on the plugin tells Core to skip the 8 byte Anchor
discriminator first. Past that it's exactly the bytes Core's own `OracleValidation`
would be.

`crank_oracle` is permissionless. It checks the hour off the Clock sysvar and sets
transfer to `Pass` from 9:00 to 17:00 UTC and `Rejected` the rest of the time. If your
crank actually flipped the state, and it's within 5 minutes of 9:00 or 17:00, the vault
pays you 0.001 SOL. If either one isn't true you get nothing, otherwise people would spam
it at noon and drain the vault.

`transfer_asset` is a `TransferV1` CPI with the oracle account passed as a remaining
account. Core won't run the check without being able to see it.

---

## Tests

19 in `tests/nft-staking.ts`.

Happy path: collection starts at `total_staked = 0` with the oracle on it, stake freezes
it and adds the burn delegate, claim pays and the NFT stays frozen, unstake pays the rest
and cleans up, burn pays the bonus and the NFT is gone, transfer during market hours.

Fail path: transferring a staked NFT, claiming before a full day, someone else claiming
your rewards, unstaking too early, burning someone else's NFT, transferring after hours.

Boundaries:

- claim at 1 day minus 1 second fails, exactly 1 day passes. `full_days > 0`.
- unstake 1 second before the freeze period fails, exactly on it passes. `>=`.
- 16:59:59 open, 17:00:00 closed, 08:59:59 closed, 09:00:00 open. `9 <= hour < 17`.
- crank reward at 5 minutes exactly pays, 5 minutes and 1 second doesn't. `<= 300`.
- cranking twice in the window only pays once.

---

## Two things that ate my afternoon

Burning a staked NFT. I figured the BurnDelegate is the whole point, it should just burn.
Nope, FreezeDelegate rejects burn too, and a reject beats an approve. So
`burn_staked_nft` has to thaw it first and then burn it, in the same instruction.

The oracle layout. Core doesn't go through Anchor to read the oracle, it just reads bytes.
I couldn't put Core's `OracleValidation` type straight in an Anchor account because it
doesn't implement the IDL stuff, so I made my own enum in the same order
(Approved, Rejected, Pass) and a `version` byte in front for the V1 tag. If that enum
order were off by one, Pass would read as Rejected and nothing would ever transfer.
