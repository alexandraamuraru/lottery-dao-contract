// Code generated by the multiversx-sc build system. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

// Init:                                 1
// Endpoints:                            8
// Async Callback (empty):               1
// Total number of exported functions:  10

#![no_std]
#![allow(internal_features)]
#![feature(lang_items)]

multiversx_sc_wasm_adapter::allocator!();
multiversx_sc_wasm_adapter::panic_handler!();

multiversx_sc_wasm_adapter::endpoints! {
    my_lottery
    (
        init => init
        start => start
        createLotteryPool => create_lottery_pool
        buy_ticket => buy_ticket
        determine_winner => determine_winner
        getActiveLotteries => get_active_lotteries
        status => status
        getLotteryInfo => lottery_info
        setRewardsDistributionAddress => set_rewards_distribution_address
    )
}

multiversx_sc_wasm_adapter::async_callback_empty! {}
