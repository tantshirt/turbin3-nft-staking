use crate::error::StakingError;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{mint_to, MintTo};
use mpl_core::{
    fetch_collection_plugin,
    instructions::UpdateCollectionPluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType},
};

// read total_staked off the collection, add or subtract one, write it back
pub fn change_total_staked<'info>(
    add: bool,
    collection: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    core_program: &AccountInfo<'info>,
    update_authority_bump: u8,
) -> Result<()> {
    let (_, attributes, _) =
        fetch_collection_plugin::<Attributes>(collection, PluginType::Attributes)
            .map_err(|_| StakingError::MissingCounter)?;

    let mut found = false;
    let mut new_list: Vec<Attribute> = Vec::new();
    for attr in attributes.attribute_list {
        if attr.key == "total_staked" {
            found = true;
            let count: u64 = attr.value.parse().map_err(|_| StakingError::MissingCounter)?;
            let count = if add {
                count.checked_add(1).ok_or(StakingError::Overflow)?
            } else {
                count.checked_sub(1).ok_or(StakingError::CounterUnderflow)?
            };
            new_list.push(Attribute { key: attr.key, value: count.to_string() });
        } else {
            new_list.push(attr);
        }
    }
    require!(found, StakingError::MissingCounter);

    let collection_key = collection.key();
    let seeds: &[&[u8]] = &[b"update_authority", collection_key.as_ref(), &[update_authority_bump]];

    UpdateCollectionPluginV1CpiBuilder::new(core_program)
        .collection(collection)
        .payer(payer)
        .authority(Some(update_authority))
        .system_program(system_program)
        .plugin(Plugin::Attributes(Attributes { attribute_list: new_list }))
        .invoke_signed(&[seeds])?;

    Ok(())
}

// config PDA is the mint authority, so it signs
pub fn mint_rewards<'info>(
    config: &Account<'info, Config>,
    reward_mint: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let seeds: &[&[u8]] = &[b"config", &[config.bump]];
    mint_to(
        CpiContext::new_with_signer(
            token_program.key(),
            MintTo {
                mint: reward_mint.clone(),
                to: to.clone(),
                authority: config.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )
}
