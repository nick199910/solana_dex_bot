use anchor_lang::account;
use anchor_lang::prelude::Pubkey;
use anchor_lang::AnchorDeserialize;
use anchor_lang::AnchorSerialize;
use rust_decimal::prelude::*;
use rust_decimal::MathematicalOps;

use crate::constant::{MAX_TICK_INDEX, MIN_TICK_INDEX, NUM_REWARDS, TICK_ARRAY_SIZE, TICK_ARRAY_SIZE_USIZE};

// #[zero_copy(unsafe)]
#[repr(C, packed)]
#[derive(Default, Copy, Clone, AnchorDeserialize, Debug, PartialEq)]
pub struct Tick {
    // Total 137 bytes
    pub initialized: bool,     // 1
    pub liquidity_net: i128,   // 16
    pub liquidity_gross: u128, // 16

    // Q64.64
    pub fee_growth_outside_a: u128, // 16
    // Q64.64
    pub fee_growth_outside_b: u128, // 16

    // Array of Q64.64
    pub reward_growths_outside: [u128; NUM_REWARDS], // 48 = 16 * 3
}

#[repr(C, packed)]
#[derive(Copy, Clone, AnchorDeserialize)]
pub struct TickArray {
    pub start_tick_index: i32,
    pub ticks: [Tick; TICK_ARRAY_SIZE_USIZE],
    pub whirlpool: Pubkey,
}



/// Stores the state relevant for tracking liquidity mining rewards at the `Whirlpool` level.
/// These values are used in conjunction with `PositionRewardInfo`, `Tick.reward_growths_outside`,
/// and `Whirlpool.reward_last_updated_timestamp` to determine how many rewards are earned by open
/// positions.
#[derive(Copy, Clone, AnchorDeserialize, AnchorSerialize, Default, Debug, PartialEq)]
pub struct WhirlpoolRewardInfo {
    /// Reward token mint.
    pub mint: Pubkey,
    /// Reward vault token account.
    pub vault: Pubkey,
    /// Authority account that has permission to initialize the reward and set emissions.
    pub authority: Pubkey,
    /// Q64.64 number that indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: u128,
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub growth_global_x64: u128,
}

#[account]
#[derive(Default, Debug, PartialEq)]
pub struct Whirlpool {
    pub whirlpools_config: Pubkey, // 32
    pub whirlpool_bump: [u8; 1],   // 1

    pub tick_spacing: u16,          // 2
    pub tick_spacing_seed: [u8; 2], // 2

    // Stored as hundredths of a basis point
    // u16::MAX corresponds to ~6.5%
    pub fee_rate: u16, // 2

    // Portion of fee rate taken stored as basis points
    pub protocol_fee_rate: u16, // 2

    // Maximum amount that can be held by Solana account
    pub liquidity: u128, // 16

    // MAX/MIN at Q32.64, but using Q64.64 for rounder bytes
    // Q64.64
    pub sqrt_price: u128,        // 16
    pub tick_current_index: i32, // 4

    pub protocol_fee_owed_a: u64, // 8
    pub protocol_fee_owed_b: u64, // 8

    pub token_mint_a: Pubkey,  // 32
    pub token_vault_a: Pubkey, // 32

    // Q64.64
    pub fee_growth_global_a: u128, // 16

    pub token_mint_b: Pubkey,  // 32
    pub token_vault_b: Pubkey, // 32

    // Q64.64
    pub fee_growth_global_b: u128, // 16

    pub reward_last_updated_timestamp: u64, // 8

    pub reward_infos: [WhirlpoolRewardInfo; NUM_REWARDS], // 384
}

fn div_floor(a: i32, b: i32) -> i32 {
    if a < 0 && a%b != 0 { a / b - 1 } else { a / b }
}


fn pricemath_sqrt_price_x64_to_price(sqrt_price_x64: u128, decimals_a: i8, decimals_b: i8) -> String {

    println!("sqrt_price_x64 is {}", sqrt_price_x64.to_string());
    let sqrt_price_x64_decimal = Decimal::from_str(&sqrt_price_x64.to_string()).unwrap();
  
    let price = sqrt_price_x64_decimal
      .checked_div(Decimal::TWO.powu(64)).unwrap()
      .powu(2)
      .checked_mul(Decimal::TEN.powi((decimals_a - decimals_b) as i64)).unwrap();
    
    price.to_string()
}

fn tickutil_get_start_tick_index(tick_current_index: i32, tick_spacing: u16, offset: i32) -> i32 {
    let ticks_in_array = TICK_ARRAY_SIZE * tick_spacing as i32;
    let real_index = div_floor(tick_current_index, ticks_in_array);
    let start_tick_index = (real_index + offset) * ticks_in_array;
  
    assert!(MIN_TICK_INDEX <= start_tick_index);
    assert!(start_tick_index + ticks_in_array <= MAX_TICK_INDEX);
    start_tick_index
}


fn pdautil_get_tick_array(program_id: &solana_sdk::pubkey::Pubkey, whirlpool_pubkey: &solana_sdk::pubkey::Pubkey, start_tick_index: i32) -> solana_sdk::pubkey::Pubkey {
    let start_tick_index_str = start_tick_index.to_string();
    let seeds = [
      b"tick_array",
      whirlpool_pubkey.as_ref(),
      start_tick_index_str.as_bytes(),
    ];
    let (pubkey, _bump) = solana_sdk::pubkey::Pubkey::find_program_address(&seeds, program_id);
    pubkey
}

fn poolutil_get_tick_array_pubkeys_for_swap(
    tick_current_index: i32,
    tick_spacing: u16,
    a_to_b: bool,
    program_id: &solana_sdk::pubkey::Pubkey,
    whirlpool_pubkey: &solana_sdk::pubkey::Pubkey,
  ) -> [solana_sdk::pubkey::Pubkey; 3] {

    let mut offset = 0;
    let mut pubkeys: [solana_sdk::pubkey::Pubkey; 3] = Default::default();
  
    for i in 0..pubkeys.len() {
      let start_tick_index = tickutil_get_start_tick_index(tick_current_index, tick_spacing, offset);
      let tick_array_pubkey = pdautil_get_tick_array(program_id, whirlpool_pubkey, start_tick_index);
      pubkeys[i] = tick_array_pubkey;
      offset = if a_to_b { offset - 1 } else { offset + 1 };
    }
    pubkeys
}

#[tokio::test]
async fn test_tick_array() {
    use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
    use solana_sdk::commitment_config::CommitmentConfig;

    let sol_usdc_whirlpool_address = solana_sdk::pubkey::Pubkey::from_str("HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ").unwrap();

    let ORCA_WHIRLPOOL_PROGRAM_ID = solana_sdk::pubkey::Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap();

    let SOL_DECIMALS = 9;
    let USDC_DECIMALS = 6;


    let rpc_url = "https://api.mainnet-beta.solana.com/";
    let rpc_client = AsyncRpcClient::new_with_commitment(rpc_url.to_string(), 
    CommitmentConfig::confirmed());

    let mut whirlpool_data = rpc_client.get_account_data(&sol_usdc_whirlpool_address).await.unwrap();

    
    // let whirlpool: Whirlpool = AnchorDeserialize::deserialize(&mut whirlpool_data.as_slice()).unwrap();
    // println!("{:?}", whirlpool);

    //   // calcu price with rust_decimal crate (at client-side)
    // println!("whirlpool price {}", pricemath_sqrt_price_x64_to_price(whirlpool.sqrt_price, SOL_DECIMALS, USDC_DECIMALS));
    // let a_to_b = true;
    // // get tickarray for swap
    // let tick_arrays = poolutil_get_tick_array_pubkeys_for_swap(
    //     whirlpool.tick_current_index,
    //     whirlpool.tick_spacing,
    //     a_to_b,
    //     &ORCA_WHIRLPOOL_PROGRAM_ID,
    //     &sol_usdc_whirlpool_address,
    // );
    // println!("tick_arrays[0] {}", tick_arrays[0].to_string());
    // println!("tick_arrays[1] {}", tick_arrays[1].to_string());
    // println!("tick_arrays[2] {}", tick_arrays[2].to_string());

    // let mut ta0_data: &[u8] = &rpc_client.get_account_data(&tick_arrays[0]).await.unwrap();
    // let mut ta1_data: &[u8] = &rpc_client.get_account_data(&tick_arrays[1]).await.unwrap();
    // let mut ta2_data: &[u8] = &rpc_client.get_account_data(&tick_arrays[2]).await.unwrap();
    // let ta0: TickArray = AnchorDeserialize::deserialize(&mut ta0_data).unwrap();
    // let ta1: TickArray = AnchorDeserialize::deserialize(&mut ta1_data).unwrap();
    // let ta2: TickArray = AnchorDeserialize::deserialize(&mut ta2_data).unwrap();

    // let ta0_start_tick_index = ta0.start_tick_index;
    // let ta1_start_tick_index = ta1.start_tick_index;
    // let ta2_start_tick_index = ta2.start_tick_index;

    // println!("ta0 start_tick_index {:?}", ta0_start_tick_index);
    // println!("ta1 start_tick_index {}", ta1_start_tick_index);
    // println!("ta2 start_tick_index {}", ta2_start_tick_index);

}