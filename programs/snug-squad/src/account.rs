use anchor_lang::prelude::*;

use crate::constants::*;

#[account]
#[derive(Default)]
pub struct PoolConfig {
    // 1
    pub is_initialized: bool,
    /// admin pubkey
    pub admin: Pubkey,
    /// Paused state of the program
    pub paused: bool,
    /// nft lock period
    pub lock_day_by_class: [u16; CLASS_TYPES],
    /// Mint of the reward token.
    pub reward_mint: Pubkey,
    /// Mint of the reward token.
    pub reward_decimal: u32,
    /// Vault to store reward tokens.
    pub reward_vault: Pubkey,
    /// The last time reward states were updated.
    pub last_update_time: i64,
    /// Tokens Staked
    pub staked_nft: u32,
    /// Reward amount per day according to class type
    pub reward_policy_by_class: [u16; CLASS_TYPES],
    /// additional reward per day by nft rarity
    pub reward_by_rarity: [u16; RARITY_TYPES],
}

impl PoolConfig {
    pub fn get_reward_per_day(&mut self, class_id: u8, rarity_id: u8) -> Result<u16> {
        let mut reward_per_day: u16 = self.reward_policy_by_class[class_id as usize];
        reward_per_day += self.reward_by_rarity[rarity_id as usize];

        Ok(reward_per_day)
    }
}

#[account]
#[derive(Default)]
pub struct StakeInfo {
    pub class_id: u32, //4
    pub owner: Pubkey, //32
    pub nft_addr: Pubkey, //32
    pub rarity_id: u32, //4
    pub stake_time: i64, //8
    pub last_update_time: i64, //8
    pub is_unstaked: u32, //4
}

impl StakeInfo {
    pub fn update_reward(&mut self, now: i64, reward_per_day: u16, reward_decimal: u32) -> Result<u64> {
        let mut last_reward_time = self.last_update_time;
        if last_reward_time < self.stake_time {
            last_reward_time = self.stake_time;
        }

        let unit_amount = (10 as u64).pow(reward_decimal);
        let reward = (unit_amount as u128)
            .checked_mul((now as u128).checked_sub(last_reward_time as u128).unwrap())
            .unwrap()
            .checked_mul(reward_per_day as u128)
            .unwrap()
            // .checked_div(REWARD_DENOMIATOR as u128)
            // .unwrap()
            .checked_div(DAY as u128)
            .unwrap() as u64;
        // reward = (((now - last_reward_time) / DAY) as u64) * reward_per_day;

        Ok(reward)
    }
}

#[account]
#[derive(Default)]
pub struct UserState {
    pub user: Pubkey, //32
    pub pending_reward: u64, //8
}
