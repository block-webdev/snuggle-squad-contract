use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, Transfer},
};
use std::mem::size_of;

pub mod account;
pub mod constants;
pub mod error;

use account::*;
use constants::*;
use error::*;

declare_id!("2FkXuxdBuEPqg5K2doi3hEKLvN9Eabben71cuzpwHvvT");

#[program]
pub mod snug_squad {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>,
        reward_policy_by_class: [u16; CLASS_TYPES],
        lock_day_by_class: [u16; CLASS_TYPES],
        reward_by_rarity: [u16; RARITY_TYPES],
        reward_decimal: u32,) -> Result<()> {
        msg!("initializing");

        let pool_account = &mut ctx.accounts.pool_account;

        pool_account.is_initialized = true;
        pool_account.admin = *ctx.accounts.admin.key;
        pool_account.paused = false; // initial status is paused
        pool_account.reward_mint = *ctx.accounts.reward_mint.to_account_info().key;
        pool_account.reward_decimal = reward_decimal;
        pool_account.reward_vault = ctx.accounts.reward_vault.key();
        pool_account.last_update_time = Clock::get()?.unix_timestamp;
        pool_account.staked_nft = 0;
        pool_account.lock_day_by_class = lock_day_by_class;
        pool_account.reward_policy_by_class = reward_policy_by_class;
        pool_account.reward_by_rarity = reward_by_rarity;

        Ok(())
    }

    pub fn stake_nft(ctx: Context<StakeNft>, class_id: u32, rarity_id: u32) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;

        // set user state key
        ctx.accounts.user_state.user = ctx.accounts.owner.key();

        // set stake info
        let staking_info = &mut ctx.accounts.nft_stake_info_account;
        staking_info.nft_addr = ctx.accounts.nft_mint.key();
        staking_info.owner = ctx.accounts.owner.key();
        staking_info.stake_time = timestamp;
        staking_info.last_update_time = timestamp;
        staking_info.class_id = class_id;
        staking_info.rarity_id = rarity_id;
        staking_info.is_unstaked = 0;

        // set global info
        ctx.accounts.pool_account.staked_nft += 1;

        Ok(())
    }

    pub fn withdraw_nft(ctx: Context<WithdrawNft>) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let staking_info = &mut ctx.accounts.nft_stake_info_account;
        let pool_account = &mut ctx.accounts.pool_account;

        let unlock_time = staking_info
            .stake_time
            .checked_add(
                (pool_account.lock_day_by_class[staking_info.class_id as usize] as i64)
                    .checked_mul(DAY as i64)
                    .unwrap(),
            )
            .unwrap();

        require!((unlock_time < timestamp), StakingError::InvalidWithdrawTime);

        let mut reward_class_id = 0;
        if unlock_time < timestamp {
            reward_class_id = staking_info.class_id;
        }

        let reward_per_day = pool_account.get_reward_per_day(reward_class_id as u8, staking_info.rarity_id as u8)?;
        // When withdraw nft, calculate and send rewards
        let reward: u64 = staking_info.update_reward(timestamp, reward_per_day, pool_account.reward_decimal)?;

        // for reward later
        staking_info.is_unstaked = 1;

        ctx.accounts.user_state.pending_reward += reward;

        ctx.accounts.pool_account.staked_nft -= 1;

        Ok(())
    }

    pub fn admin_withdraw_nft(ctx: Context<AdminWithdrawNft>) -> Result<()> {
        ctx.accounts.nft_stake_info_account.is_unstaked = 1;
        ctx.accounts.pool_account.staked_nft -= 1;

        Ok(())
    }

    pub fn owner_withdraw_nft(ctx: Context<OwnerWithdrawNft>) -> Result<()> {
        ctx.accounts.nft_stake_info_account.is_unstaked = 1;
        ctx.accounts.pool_account.staked_nft -= 1;

        Ok(())
    }

    #[access_control(user(&ctx.accounts.nft_stake_info_account, &ctx.accounts.owner))]
    pub fn claim_reward(ctx: Context<ClaimReward>) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;
        let staking_info = &mut ctx.accounts.nft_stake_info_account;

        // calulate reward of this nft
        let pool_account = &mut ctx.accounts.pool_account;
        let reward_per_day = pool_account.get_reward_per_day(staking_info.class_id as u8, staking_info.rarity_id as u8)?;
        // When withdraw nft, calculate and send reward SWRD
        let reward: u64 = staking_info.update_reward(timestamp, reward_per_day, pool_account.reward_decimal)?;
        staking_info.last_update_time = timestamp;

        if staking_info.is_unstaked == 1 {
            // reward = staking_info.reward;
        } else {
            // let vault_balance = ctx.accounts.reward_vault.amount;
            // if vault_balance < reward {
            //     reward = vault_balance;
            // }
        }

        // Transfer rewards from the pool reward vaults to user reward vaults.
        let (_pool_account_seed, _bump) =
            Pubkey::find_program_address(&[RS_PREFIX.as_bytes()], ctx.program_id);
        // let bump = ctx.bumps.get(RS_PREFIX).unwrap();
        let pool_seeds = &[RS_PREFIX.as_bytes(), &[_bump]];
        let signer = &[&pool_seeds[..]];

        let token_program = ctx.accounts.token_program.to_account_info().clone();
        let token_accounts = anchor_spl::token::Transfer {
            from: ctx.accounts.reward_vault.to_account_info().clone(),
            to: ctx.accounts.reward_to_account.to_account_info().clone(),
            authority: ctx.accounts.pool_account.to_account_info().clone(),
        };
        let cpi_ctx = CpiContext::new(token_program, token_accounts);
        msg!(
            "Calling the token program to transfer reward {} to the user",
            reward
        );
        anchor_spl::token::transfer(cpi_ctx.with_signer(signer), reward)?;

        Ok(())
    }

    pub fn claim_pending_reward(ctx: Context<ClaimPendingReward>) -> Result<()> {
        let pending_reward = ctx.accounts.user_state.pending_reward;
        if pending_reward > 0 {
            ctx.accounts.user_state.pending_reward = 0;

            // Transfer rewards from the pool reward vaults to user reward vaults.
            let (_pool_account_seed, _bump) =
                Pubkey::find_program_address(&[RS_PREFIX.as_bytes()], ctx.program_id);
            // let bump = ctx.bumps.get(RS_PREFIX).unwrap();
            let pool_seeds = &[RS_PREFIX.as_bytes(), &[_bump]];
            let signer = &[&pool_seeds[..]];

            let token_program = ctx.accounts.token_program.to_account_info().clone();
            let token_accounts = anchor_spl::token::Transfer {
                from: ctx.accounts.reward_vault.to_account_info().clone(),
                to: ctx.accounts.reward_to_account.to_account_info().clone(),
                authority: ctx.accounts.pool_account.to_account_info().clone(),
            };
            let cpi_ctx = CpiContext::new(token_program, token_accounts);
            msg!(
                "Calling the token program to transfer reward {} to the user",
                pending_reward
            );
            anchor_spl::token::transfer(cpi_ctx.with_signer(signer), pending_reward)?;
        }

        Ok(())
    }

    pub fn deposit_reward(ctx: Context<DepositReward>, amount: u64) -> Result<()> {
        // Transfer reward tokens into the vault.
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.funder_account.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        Ok(())
    }

    pub fn withdraw_reward(ctx: Context<WithdrawReward>) -> Result<()> {
        let vault_amount = ctx.accounts.reward_vault.amount;

        if vault_amount > 0 {
            let (_pool_account_seed, _bump) =
                Pubkey::find_program_address(&[RS_PREFIX.as_bytes()], ctx.program_id);

            // let _bump = ctx.bumps.get(RS_PREFIX).unwrap();
            let pool_seeds = &[RS_PREFIX.as_bytes(), &[_bump]];
            let signer = &[&pool_seeds[..]];

            let token_accounts = anchor_spl::token::Transfer {
                from: ctx.accounts.reward_vault.to_account_info().clone(),
                to: ctx.accounts.reward_to_account.to_account_info().clone(),
                authority: ctx.accounts.pool_account.to_account_info().clone(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), token_accounts);
            msg!(
                "Calling the token program to withdraw reward {} to the admin",
                vault_amount
            );
            anchor_spl::token::transfer(cpi_ctx.with_signer(signer), vault_amount)?;
        }
        Ok(())
    }

    pub fn change_pool_setting(
        ctx: Context<ChangePoolSetting>,
        reward_policy_by_class: [u16; CLASS_TYPES],
        lock_day_by_class: [u16; CLASS_TYPES],
        reward_by_rarity: [u16; RARITY_TYPES],
        paused: bool,
    ) -> Result<()> {
        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.paused = paused; // initial status is paused
        pool_account.last_update_time = Clock::get()?.unix_timestamp;
        pool_account.lock_day_by_class = lock_day_by_class;
        pool_account.reward_policy_by_class = reward_policy_by_class;
        pool_account.reward_by_rarity = reward_by_rarity;
        Ok(())
    }

    pub fn change_reward_mint(ctx: Context<ChangeRewardMint>, reward_mint: Pubkey) -> Result<()> {
        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.reward_mint = reward_mint;
        Ok(())
    }

    pub fn transfer_ownership(ctx: Context<TransferOwnership>, new_admin: Pubkey) -> Result<()> {
        let pool_account = &mut ctx.accounts.pool_account;
        pool_account.admin = new_admin;
        Ok(())
    }

    pub fn close_stake_info(_ctx: Context<CloseStakeInfo>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    // The pool owner
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        payer = admin,
        space = 8 + size_of::<PoolConfig>(),
    )]
    pub pool_account: Account<'info, PoolConfig>,

    // reward mint
    pub reward_mint: Account<'info, Mint>,

    // reward vault that holds the reward mint for distribution
    #[account(
        init,
        token::mint = reward_mint,
        token::authority = pool_account,
        seeds = [ RS_VAULT_SEED.as_bytes(), reward_mint.key().as_ref() ],
        bump,
        payer = admin,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    // The rent sysvar
    pub rent: Sysvar<'info, Rent>,
    // system program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub system_program: Program<'info, System>,

    // token program
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
// #[instruction(global_bump: u8, staked_nft_bump: u8)]
pub struct StakeNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        constraint = pool_account.is_initialized == true,
        constraint = pool_account.paused == false,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [USER_STATE_SEED, owner.key().as_ref()],
        bump,
        space = 8 + size_of::<UserState>(),
    )]
    pub user_state: Account<'info, UserState>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key.as_ref()],
        bump,
        space = 8 + size_of::<StakeInfo>(),
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,

    /// CHECK: unsafe
    pub nft_mint: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        constraint = pool_account.is_initialized == true,
        constraint = pool_account.paused == false,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [USER_STATE_SEED, owner.key().as_ref()],
        bump,
    )]
    pub user_state: Account<'info, UserState>,

    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key().as_ref()],
        bump,
        has_one = owner,
        close = owner,
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,
}

#[derive(Accounts)]
pub struct AdminWithdrawNft<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        has_one = admin,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key().as_ref()],
        bump,
        close = admin,
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,
}

#[derive(Accounts)]
pub struct OwnerWithdrawNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    pub nft_mint: Account<'info, Mint>,

    // send reward to user reward vault
    #[account(
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub nft_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key().as_ref()],
        bump,
        close = owner,
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        constraint = pool_account.is_initialized == true,
        constraint = pool_account.paused == false,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key().as_ref()],
        bump,
        // close = owner,
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,

    #[account(
        mut,
        token::mint = reward_mint,
        token::authority = pool_account,
    )]
    reward_vault: Box<Account<'info, TokenAccount>>,

    #[account(address = pool_account.reward_mint)]
    pub reward_mint: Account<'info, Mint>,

    // send reward to user reward vault
    #[account(
      init_if_needed,
      payer = owner,
      associated_token::mint = reward_mint,
      associated_token::authority = owner
    )]
    reward_to_account: Box<Account<'info, TokenAccount>>,

    pub nft_mint: Account<'info, Mint>,

    // The Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ClaimPendingReward<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        constraint = pool_account.is_initialized == true,
        constraint = pool_account.paused == false,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [USER_STATE_SEED, owner.key().as_ref()],
        bump,
    )]
    pub user_state: Account<'info, UserState>,

    #[account(
        mut,
        token::mint = reward_mint,
        token::authority = pool_account,
    )]
    reward_vault: Box<Account<'info, TokenAccount>>,

    #[account(address = pool_account.reward_mint)]
    pub reward_mint: Account<'info, Mint>,

    // send reward to user reward vault
    #[account(
      init_if_needed,
      payer = owner,
      associated_token::mint = reward_mint,
      associated_token::authority = owner
    )]
    reward_to_account: Box<Account<'info, TokenAccount>>,

    // The Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}


#[derive(Accounts)]
pub struct CloseStakeInfo<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_STAKEINFO_SEED.as_ref(), nft_mint.key().as_ref()],
        bump,
        constraint = nft_stake_info_account.owner == owner.key(),
        close = owner,
    )]
    pub nft_stake_info_account: Account<'info, StakeInfo>,

    pub nft_mint: Account<'info, Mint>,
}


#[derive(Accounts)]
pub struct DepositReward<'info> {
    #[account(mut)]
    funder: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        constraint = pool_account.is_initialized == true,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        mut,
        token::mint = reward_mint,
        token::authority = pool_account,
    )]
    reward_vault: Box<Account<'info, TokenAccount>>,

    // funder account
    #[account(mut)]
    funder_account: Account<'info, TokenAccount>,

    #[account(address = pool_account.reward_mint)]
    pub reward_mint: Box<Account<'info, Mint>>,

    // The Token Program
    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawReward<'info> {
    #[account(mut)]
    admin: Signer<'info>,
    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        has_one = admin,
    )]
    pub pool_account: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [ RS_VAULT_SEED.as_bytes(), reward_mint.key().as_ref() ],
        bump,
        token::mint = reward_mint,
        token::authority = pool_account,
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    // send reward to user reward vault
    #[account(
      init_if_needed,
      payer = admin,
      associated_token::mint = reward_mint,
      associated_token::authority = admin
    )]
    reward_to_account: Box<Account<'info, TokenAccount>>,

    // reward mint
    #[account(address = pool_account.reward_mint)]
    reward_mint: Account<'info, Mint>,

    // The Token Program
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ChangePoolSetting<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        has_one = admin,
        constraint = pool_account.is_initialized == true,
    )]
    pub pool_account: Account<'info, PoolConfig>,
}

#[derive(Accounts)]
pub struct ChangeRewardMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        has_one = admin,
        constraint = pool_account.is_initialized == true,
    )]
    pub pool_account: Account<'info, PoolConfig>,
}

#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [RS_PREFIX.as_bytes()],
        bump,
        has_one = admin,
        constraint = pool_account.is_initialized == true,
    )]
    pub pool_account: Account<'info, PoolConfig>,
}
// Access control modifiers
impl<'info> Initialize<'info> {
    pub fn validate(&self) -> Result<()> {
        if self.pool_account.is_initialized == true {
            require!(
                self.pool_account.admin.eq(&self.admin.key()),
                StakingError::NotAllowedAuthority
            )
        }
        Ok(())
    }
}

pub fn user(stake_info_account: &Account<StakeInfo>, user: &AccountInfo) -> Result<()> {
    require!(
        stake_info_account.owner == *user.key,
        StakingError::InvalidUserAddress
    );
    Ok(())
}
