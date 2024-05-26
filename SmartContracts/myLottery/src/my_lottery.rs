#![no_std]

use multiversx_sc::imports::*;


mod lottery_info;
mod lottery_status;

use lottery_info::LotteryInfo;
use lottery_status::Status;

const PERCENTAGE_TOTAL: u32 = 100;
const FIVE_MINUTES_IN_SECONDS: u64 = 60 * 5;
//const MAX_TICKETS_PER_USER: usize = 20;
const MAX_TICKETS: usize = 800; // Assuming a max cap for tickets to sell
const PRIZE_DISTRIBUTION: [u64; 3] = [40_000, 20_000, 10_000]; // 40%, 20%, 10% for 1st, 2nd, and 3rd


#[multiversx_sc::contract]
pub trait MyLottery {
    #[init]
    fn init(&self) {}

    #[allow_multiple_var_args]
    #[endpoint]
    fn start(
        &self,
        lottery_name: ManagedBuffer,
        token_identifier: EgldOrEsdtTokenIdentifier,
        ticket_price: BigUint,
        opt_total_tickets: Option<usize>,
        opt_deadline: Option<u64>,
        opt_max_entries_per_user: Option<usize>,
        opt_prize_distribution: ManagedOption<ManagedVec<u8>>,
    ) {
        self.start_lottery(
            lottery_name,
            token_identifier,
            ticket_price,
            opt_total_tickets,
            opt_deadline,
            opt_max_entries_per_user,
            opt_prize_distribution,
        );
    }

    #[allow_multiple_var_args]
    #[endpoint(createLotteryPool)]
    fn create_lottery_pool(
        &self,
        lottery_name: ManagedBuffer,
        token_identifier: EgldOrEsdtTokenIdentifier,
        ticket_price: BigUint,
        opt_total_tickets: Option<usize>,
        opt_deadline: Option<u64>,
        opt_max_entries_per_user: Option<usize>,
        opt_prize_distribution: ManagedOption<ManagedVec<u8>>,
    ) {
        self.start_lottery(
            lottery_name,
            token_identifier,
            ticket_price,
            opt_total_tickets,
            opt_deadline,
            opt_max_entries_per_user,
            opt_prize_distribution,
        );
    }

    #[allow_multiple_var_args]
    #[allow(clippy::too_many_arguments)]
    fn start_lottery(
        &self,
        lottery_name: ManagedBuffer,
        token_identifier: EgldOrEsdtTokenIdentifier,
        ticket_price: BigUint,
        opt_total_tickets: Option<usize>,
        opt_deadline: Option<u64>,
        opt_max_entries_per_user: Option<usize>,
        opt_prize_distribution: ManagedOption<ManagedVec<u8>>,
    ) {
        require!(!lottery_name.is_empty(), "Name can't be empty!");

        let timestamp = self.blockchain().get_block_timestamp();
        let total_tickets = opt_total_tickets.unwrap_or(MAX_TICKETS);
        let deadline = opt_deadline.unwrap_or(timestamp + FIVE_MINUTES_IN_SECONDS);
        let max_entries_per_user = opt_max_entries_per_user.unwrap_or(MAX_TICKETS);
        let prize_distribution = opt_prize_distribution
            .unwrap_or_else(|| ManagedVec::from_single_item(PERCENTAGE_TOTAL as u8));

        require!(
            self.status(&lottery_name) == Status::Inactive,
            "Lottery is already active!"
        );
        require!(!lottery_name.is_empty(), "Can't have empty lottery name!");
        require!(token_identifier.is_valid(), "Invalid token name provided!");
        require!(ticket_price > 0, "Ticket price must be higher than 0!");
        require!(deadline > timestamp, "Deadline can't be in the past!");
        require!(
            deadline <= timestamp + FIVE_MINUTES_IN_SECONDS,
            "Deadline can't be later than 30 days from now!"
        );
        require!(
            max_entries_per_user > 0,
            "Must have more than 0 max entries per user!"
        );
        require!(
            self.sum_array(&prize_distribution) == PERCENTAGE_TOTAL,
            "Prize distribution must add up to exactly 100(%)!"
        );
        require!(
            total_tickets > 0,
            "Must have more than 0 tickets available!"
        );
        require!(
            total_tickets <= MAX_TICKETS,
            "Only 800 or less total tickets per lottery are allowed!"
        );

        let info = LotteryInfo {
            token_identifier,
            ticket_price,
            tickets_left: total_tickets,
            deadline,
            max_entries_per_user,
            prize_distribution,
            prize_pool: BigUint::zero(),
        };

        self.lottery_info(&lottery_name).set(&info);
        self.all_lottery_names().push(&lottery_name);
    }

    #[endpoint]
    #[payable("*")]
    fn buy_ticket(&self, lottery_name: ManagedBuffer) {
        let (token_identifier, payment) = self.call_value().egld_or_single_fungible_esdt();

        match self.status(&lottery_name) {
            Status::Inactive => sc_panic!("Lottery is currently inactive."),
            Status::Running => {
                self.update_after_buy_ticket(&lottery_name, &token_identifier, &payment)
            },
            Status::Ended => {
                sc_panic!("Lottery entry period has ended! Awaiting winner announcement.")
            },
        };
    }

    #[endpoint]
    fn determine_winner(&self, lottery_name: ManagedBuffer) {
        match self.status(&lottery_name) {
            Status::Inactive => sc_panic!("Lottery is inactive!"),
            Status::Running => sc_panic!("Lottery is still running!"),
            Status::Ended => {
                self.distribute_prizes(&lottery_name);
                self.clear_storage(&lottery_name);
            },
        };
    }

    #[view(getActiveLotteries)]
    fn get_active_lotteries(&self) -> ManagedVec<ManagedBuffer> {
        let mut active_lotteries = ManagedVec::new();
        let lottery_names = self.all_lottery_names();

        for lottery_name in lottery_names.iter() {
            if self.status(&lottery_name) == Status::Running {
                active_lotteries.push(lottery_name);
            }
        }

        active_lotteries
    }

    #[view]
    fn status(&self, lottery_name: &ManagedBuffer) -> Status {
        if self.lottery_info(lottery_name).is_empty() {
            return Status::Inactive;
        }

        let info = self.lottery_info(lottery_name).get();
        let current_time = self.blockchain().get_block_timestamp();
        if current_time > info.deadline || info.tickets_left == 0 {
            return Status::Ended;
        }

        Status::Running
    }

    fn update_after_buy_ticket(
        &self,
        lottery_name: &ManagedBuffer,
        token_identifier: &EgldOrEsdtTokenIdentifier,
        payment_amount: &BigUint,
    ) {
        let info_mapper = self.lottery_info(lottery_name);
        let mut info = info_mapper.get();
        let caller = self.blockchain().get_caller();

        require!(
            token_identifier == &info.token_identifier && payment_amount == &info.ticket_price,
            "Wrong ticket fee!"
        );
        
        let entries_mapper = self.number_of_entries_for_user(lottery_name, &caller);
        let mut entries = entries_mapper.get();
        require!(
            entries < info.max_entries_per_user,
            "Ticket limit exceeded for this lottery!"
        );

        self.ticket_holders(lottery_name).push(&caller);

        entries += 1;
        info.tickets_left -= 1;
        info.prize_pool += &info.ticket_price;

        entries_mapper.set(entries);
        info_mapper.set(&info);
    } 

    fn distribute_prizes(&self, lottery_name: &ManagedBuffer) {
        let info = self.lottery_info(lottery_name).get();
        let ticket_holders_mapper = self.ticket_holders(lottery_name);
        let total_tickets = ticket_holders_mapper.len();
        let total_prize = info.prize_pool.clone();
        let dao_rewards_percentage = 30u32;
    
        if total_tickets == 0 {
            return;
        }
    
        let winning_ticket_indices = self.get_distinct_random(1, total_tickets, 3);
        let mut total_distributed_prize = BigUint::zero();
    
        for (index, &percentage) in PRIZE_DISTRIBUTION.iter().enumerate() {
            let winner_address = ticket_holders_mapper.get(winning_ticket_indices[index]);
            let prize = total_prize.clone() * BigUint::from(percentage) / BigUint::from(100u32);
            total_distributed_prize += &prize;
    
            self.send().direct(&winner_address, &info.token_identifier, 0, &prize);
        }
    
        let base_dao_rewards = total_prize.clone() * BigUint::from(dao_rewards_percentage) / BigUint::from(100u32);

        let remainder = &total_prize - &total_distributed_prize - &base_dao_rewards;
        let dao_rewards = base_dao_rewards + remainder;

        let rewards_contract_address = self.rewards_distribution_address().get();
        self.send().direct_with_gas_limit(
            &rewards_contract_address,
            &info.token_identifier,
            0,
            &dao_rewards,
            500_000,
            "receiveRewards",
            &[],
        );
    }
    

    fn clear_storage(&self, lottery_name: &ManagedBuffer) {
        let mut ticket_holders_mapper = self.ticket_holders(lottery_name);
        let current_ticket_number = ticket_holders_mapper.len();

        for i in 1..=current_ticket_number {
            let addr = ticket_holders_mapper.get(i);
            self.number_of_entries_for_user(lottery_name, &addr).clear();
        }

        ticket_holders_mapper.clear();
        self.lottery_info(lottery_name).clear();
    }

    fn sum_array(&self, array: &ManagedVec<u8>) -> u32 {
        let mut sum = 0;

        for item in array {
            sum += item as u32;
        }

        sum
    }

    fn get_distinct_random(
        &self,
        min: usize,
        max: usize,
        amount: usize,
    ) -> ArrayVec<usize, MAX_TICKETS> {
        let mut selected_indices = ArrayVec::<usize, MAX_TICKETS>::new();
        let mut attempts = 0usize;

        let mut rand = RandomnessSource::new();

        while selected_indices.len() < amount && attempts < amount * 10 {
            let rand_index = rand.next_usize_in_range(min, max + 1);
    
            if !selected_indices.contains(&rand_index) {
                selected_indices.push(rand_index);
            }
    
            attempts += 1;
        }
    
        selected_indices
    }    

    // storage

    #[view(getLotteryInfo)]
    #[storage_mapper("lotteryInfo")]
    fn lottery_info(
        &self,
        lottery_name: &ManagedBuffer,
    ) -> SingleValueMapper<LotteryInfo<Self::Api>>;

    #[storage_mapper("allLotteryNames")]
    fn all_lottery_names(&self) -> VecMapper<ManagedBuffer>;

    #[storage_mapper("ticketHolder")]
    fn ticket_holders(&self, lottery_name: &ManagedBuffer) -> VecMapper<ManagedAddress>;

    #[storage_mapper("numberOfEntriesForUser")]
    fn number_of_entries_for_user(
        &self,
        lottery_name: &ManagedBuffer,
        user: &ManagedAddress,
    ) -> SingleValueMapper<usize>;

    #[storage_mapper("rewardsDistributionAddress")]
    fn rewards_distribution_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[only_owner]
    #[endpoint(setRewardsDistributionAddress)]
    fn set_rewards_distribution_address(&self, address: ManagedAddress) {
        self.rewards_distribution_address().set(&address);
    }

}





