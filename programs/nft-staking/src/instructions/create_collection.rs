use crate::state::{Config, Oracle};
use anchor_lang::prelude::*;
use mpl_core::{
    instructions::CreateCollectionV2CpiBuilder,
    types::{
        Attribute, Attributes, ExternalCheckResult, ExternalPluginAdapterInitInfo,
        HookableLifecycleEvent, OracleInitInfo, Plugin, PluginAuthority, PluginAuthorityPair,
        ValidationResultsOffset,
    },
};

#[derive(Accounts)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [b"config"], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,

    // brand new keypair, core creates it
    #[account(mut)]
    pub collection: Signer<'info>,

    /// CHECK: just a PDA that signs as the collection's update authority, holds no data
    #[account(seeds = [b"update_authority", collection.key().as_ref()], bump)]
    pub update_authority: UncheckedAccount<'info>,

    #[account(seeds = [b"oracle"], bump = oracle.bump)]
    pub oracle: Account<'info, Oracle>,

    /// CHECK: address pinned to the core program
    #[account(address = mpl_core::ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> CreateCollection<'info> {
    pub fn create_collection(
        &mut self,
        name: String,
        uri: String,
        bumps: &CreateCollectionBumps,
    ) -> Result<()> {
        self.config.collection = self.collection.key();
        self.config.update_authority_bump = bumps.update_authority;

        // the counter lives on the collection itself, as a string because attributes are strings
        let attributes = PluginAuthorityPair {
            plugin: Plugin::Attributes(Attributes {
                attribute_list: vec![Attribute {
                    key: "total_staked".to_string(),
                    value: "0".to_string(),
                }],
            }),
            authority: Some(PluginAuthority::UpdateAuthority),
        };

        // flags is a bitfield: listen = 1, approve = 2, reject = 4. we only want reject
        let oracle = ExternalPluginAdapterInitInfo::Oracle(OracleInitInfo {
            base_address: self.oracle.key(),
            init_plugin_authority: None,
            lifecycle_checks: vec![(
                HookableLifecycleEvent::Transfer,
                ExternalCheckResult { flags: 4 },
            )],
            base_address_config: None,
            // skip the 8 byte anchor discriminator when core reads the oracle
            results_offset: Some(ValidationResultsOffset::Anchor),
        });

        CreateCollectionV2CpiBuilder::new(&self.core_program.to_account_info())
            .collection(&self.collection.to_account_info())
            .update_authority(Some(&self.update_authority.to_account_info()))
            .payer(&self.admin.to_account_info())
            .system_program(&self.system_program.to_account_info())
            .name(name)
            .uri(uri)
            .plugins(vec![attributes])
            .external_plugin_adapters(vec![oracle])
            .invoke()?;

        Ok(())
    }
}
