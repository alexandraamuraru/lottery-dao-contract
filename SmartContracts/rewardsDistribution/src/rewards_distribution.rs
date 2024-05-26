#![no_std]

use multiversx_sc::{imports::*};

#[multiversx_sc::contract]
pub trait RewardsDistribution {
    #[init]
    fn init(&self) {}

    #[payable("*")]
    #[endpoint(receiveRewards)]
    fn receive_rewards(&self) {
        let (token_identifier, amount) = self.call_value().egld_or_single_fungible_esdt();
        // Update the rewards pool
        self.rewards_pool(&token_identifier).update(|val| *val += &amount);
    }

    #[endpoint(distributeRewards)]
    fn distribute_rewards(&self, token_identifier: EgldOrEsdtTokenIdentifier) {
        let total_rewards = self.rewards_pool(&token_identifier).get();
        let total_supply = self.calculate_total_supply();

        for holder in self.token_holders().keys() {
            let holder_balance = self.token_holders().get(&holder).unwrap_or_default();
            let holder_share = total_rewards.clone() * holder_balance.clone() / total_supply.clone();
            self.pending_rewards(&holder, &token_identifier).update(|val| *val += &holder_share);
        }

        // Clear the rewards pool after distribution
        self.rewards_pool(&token_identifier).clear();
    }

    #[endpoint(claimRewards)]
    fn claim_rewards(&self, token_identifier: EgldOrEsdtTokenIdentifier) {
        let caller = self.blockchain().get_caller();
        let claimable_rewards = self.pending_rewards(&caller, &token_identifier).get();
        require!(claimable_rewards > 0, "No rewards to claim");

        self.send().direct(&caller, &token_identifier, 0, &claimable_rewards);
        self.pending_rewards(&caller, &token_identifier).clear();
    }

    // Helper function to calculate the total supply of tokens held by all holders
    fn calculate_total_supply(&self) -> BigUint {
        let mut total_supply = BigUint::zero();
        for balance in self.token_holders().values() {
            total_supply += balance;
        }
        total_supply
    }

    #[storage_mapper("rewardsPool")]
    fn rewards_pool(&self, token_identifier: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;

    #[storage_mapper("tokenHolders")]
    fn token_holders(&self) -> MapMapper<ManagedAddress, BigUint>;

    #[storage_mapper("pendingRewards")]
    fn pending_rewards(&self, holder: &ManagedAddress, token_identifier: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getTokenHolders)]
    fn get_token_holders(&self) -> MultiValue2<ManagedVec<ManagedAddress>, ManagedVec<BigUint>> {
        let mut addresses = ManagedVec::new();
        let mut balances = ManagedVec::new();

        for (address, balance) in self.token_holders().iter() {
            addresses.push(address);
            balances.push(balance);
        }

        MultiValue2::from((addresses, balances))
    }

    // Additional methods to manage token holders
    #[endpoint(updateHolderBalance)]
    fn update_holder_balance(&self, holder: ManagedAddress, balance: BigUint) {
        self.token_holders().insert(holder, balance);
    }

    #[endpoint(removeHolder)]
    fn remove_holder(&self, holder: ManagedAddress) {
        self.token_holders().remove(&holder);
    }
}
