// extern crate dotenv;

use constant::{ORCA_WHIRLPOOL_PROGRAM_ID, USDC_ADDRESS, USDC_DECIMALS, WSOL_ADDRESS, WSOL_DECIMALS, WSOL_USDC_3000};
use dotenv::dotenv;
use tick_array::{calculate_token_b_amount, generate_oracle_pda, poolutil_get_tick_array_pubkeys_for_swap, pricemath_sqrt_price_x64_to_price, Whirlpool};
use std::env;

mod tick_array;
mod constant;


use anchor_lang::prelude::*;
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use std::str::FromStr;

pub struct DEXClient {
    rpc_client: AsyncRpcClient,
}

#[derive(Default, AnchorSerialize, AnchorDeserialize)]
pub struct SwapData {
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,
}

#[derive(Default)]
pub struct SwapTokenData {
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub token_in: solana_sdk::pubkey::Pubkey, 
    pub token_out: solana_sdk::pubkey::Pubkey, 
    pub token_in_decimals: i8, 
    pub token_out_decimals: i8, 
    pub slippage: u16,
}

impl SwapTokenData {
    pub fn new(amount: u64, token_in: solana_sdk::pubkey::Pubkey, token_out: solana_sdk::pubkey::Pubkey, token_in_decimals: i8, token_out_decimals: i8, slippage: u16) -> Self {
        SwapTokenData {
            amount,
            other_amount_threshold: 0,
            sqrt_price_limit: 0,
            amount_specified_is_input: true,
            token_in,
            token_out,
            token_in_decimals,
            token_out_decimals,
            slippage,
        }
    }
}




impl DEXClient {
    pub fn new(rpc_url: &str) -> Self {
        let rpc_client = AsyncRpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        DEXClient { rpc_client }
    }

    pub async fn orca_swap(
        &self,
        user: &Keypair,
        whirlpool: &Pubkey,
        token_authority: &Pubkey,
        token_owner_account_a: &Pubkey,
        token_vault_a: &Pubkey,
        token_owner_account_b: &Pubkey,
        token_vault_b: &Pubkey,
        tick_array_0: &Pubkey,
        tick_array_1: &Pubkey,
        tick_array_2: &Pubkey,
        oracle: &Pubkey,
        amount: u64,
        other_amount_threshold: u64,
        sqrt_price_limit: u128,
        amount_specified_is_input: bool,
        a_to_b: bool,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let orca_swap_program_id = Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM_ID)?;
        let token_program = spl_token::id();

        let accounts = vec![
            AccountMeta::new(token_program, false),
            AccountMeta::new(*token_authority, true),
            AccountMeta::new(*whirlpool, false),
            AccountMeta::new(*token_owner_account_a, false),
            AccountMeta::new(*token_vault_a, false),
            AccountMeta::new(*token_owner_account_b, false),
            AccountMeta::new(*token_vault_b, false),
            AccountMeta::new(*tick_array_0, false),
            AccountMeta::new(*tick_array_1, false),
            AccountMeta::new(*tick_array_2, false),
            AccountMeta::new(*oracle, false),
        ];

        let swap_data = SwapData {
            amount,
            other_amount_threshold,
            sqrt_price_limit,
            amount_specified_is_input,
            a_to_b,
        };

        let data = {
            let mut prefix = vec![248, 198, 158, 145, 225, 117, 135, 200];
            prefix.extend(swap_data.try_to_vec()?);
            prefix
        };

        let instruction = Instruction {
            program_id: orca_swap_program_id,
            accounts,
            data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;

        println!("recent_blockhash: {}", recent_blockhash);

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&user.pubkey()),
            &[user],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction).await?;
        Ok(signature.to_string())
    }


    pub async fn send_instructions(
        &self,
        user: &Keypair,
        instructions: Vec<Instruction>,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        println!("recent_blockhash: {}", recent_blockhash);
    
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&user.pubkey()),
            &[user],
            recent_blockhash,
        );
    
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction).await?;
        Ok(signature.to_string())
    }



    pub async fn get_or_create_associated_token_account(&self, user: &Keypair, mint: &Pubkey) -> std::result::Result<Pubkey, Box<dyn std::error::Error>> {
        let associated_token_address = get_associated_token_address(&user.pubkey(), mint);
        
        match self.rpc_client.get_account(&associated_token_address).await {
            Ok(_) => {
                Ok(associated_token_address)
            },
            Err(_) => {
                println!("Creating new associated token account");
                self.create_associated_token_account(user, mint).await
            }
        }
    }

    async fn create_associated_token_account(&self, user: &Keypair, mint: &Pubkey) -> std::result::Result<Pubkey, Box<dyn std::error::Error>> {
        let associated_token_address = get_associated_token_address(&user.pubkey(), mint);
        
        let instruction = spl_associated_token_account::instruction::create_associated_token_account(
            &user.pubkey(),
            &user.pubkey(),
            mint,
            &spl_token::id(),
        );

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&user.pubkey()),
            &[user],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction).await?;

        Ok(associated_token_address)
    }

    pub async fn get_token_balance(&self, token_account: &Pubkey) -> std::result::Result<u64, Box<dyn std::error::Error>> {
        let account = self.rpc_client.get_token_account_balance(token_account).await?;
        Ok(account.amount.parse()?)
    }
}




async fn build_orca_pool_swap_instruction(orca_client: &DEXClient, user: &Keypair, swap_token_data: &SwapTokenData, pool_address: &solana_sdk::pubkey::Pubkey, dex_address: &solana_sdk::pubkey::Pubkey, slippage: u16) -> std::result::Result<Instruction, Box<dyn std::error::Error>> {
 


    let mut whirlpool_data = orca_client.rpc_client.get_account_data(pool_address).await.unwrap();
    let whirlpool_data_slice = &mut whirlpool_data.as_mut_slice()[8..].as_ref();
    
    let whirlpool: Whirlpool = AnchorDeserialize::deserialize(whirlpool_data_slice).unwrap();

    let a_to_b = if swap_token_data.token_in.eq(&solana_sdk::pubkey::Pubkey::try_from_slice(&whirlpool.token_mint_a.to_bytes()).unwrap()) {
        true
    } else {
        false
    };

    // calcu price with rust_decimal crate (at client-side)
    let token_in_price = pricemath_sqrt_price_x64_to_price(whirlpool.sqrt_price, swap_token_data.token_in_decimals, swap_token_data.token_out_decimals);

    println!("token_in_price : {}", token_in_price);


    let token_out_amount_min = calculate_token_b_amount(&token_in_price, swap_token_data.amount, slippage, whirlpool.fee_rate, whirlpool.protocol_fee_rate);

    println!("token_out_amount_min {:?}", token_out_amount_min);

    
    let user_token_a_account = orca_client.get_or_create_associated_token_account(&user, &solana_sdk::pubkey::Pubkey::try_from_slice(&whirlpool.token_mint_a.to_bytes()).unwrap()).await?;

    let user_token_b_account = orca_client.get_or_create_associated_token_account(&user, &solana_sdk::pubkey::Pubkey::try_from_slice(&whirlpool.token_mint_b.to_bytes()).unwrap()).await?;
    
   
    let tick_arrays = poolutil_get_tick_array_pubkeys_for_swap(
        whirlpool.tick_current_index,
        whirlpool.tick_spacing,
        a_to_b,
        &dex_address,
        &pool_address,
    );
    
    let token_vault_a = solana_sdk::pubkey::Pubkey::try_from_slice(&whirlpool.token_vault_a.to_bytes()).unwrap();
    let token_vault_b = solana_sdk::pubkey::Pubkey::try_from_slice(&whirlpool.token_vault_b.to_bytes()).unwrap();
    let tick_array_0 = tick_arrays[0];
    let tick_array_1 = tick_arrays[1];
    let tick_array_2 = tick_arrays[2];
    let (oracle, _) = generate_oracle_pda(pool_address, dex_address);
    
    let token_a_balance_before = orca_client.get_token_balance(&user_token_a_account).await?;
    let token_b_balance_before = orca_client.get_token_balance(&user_token_b_account).await?;

    if a_to_b {
        println!("token_a_balance_before : {}", token_a_balance_before);
        println!("token_b_balance_before : {}", token_b_balance_before);
    } else {
        println!("token_a_balance_before : {}", token_b_balance_before);
        println!("token_b_balance_before : {}", token_a_balance_before);
    }


    let orca_swap_program_id = solana_sdk::pubkey::Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM_ID).unwrap();
    let token_program = spl_token::id();

    

    let accounts = vec![
        AccountMeta::new(token_program, false),
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(*pool_address, false),
        AccountMeta::new(user_token_a_account, false),
        AccountMeta::new(token_vault_a, false),
        AccountMeta::new(user_token_b_account, false),
        AccountMeta::new(token_vault_b, false),
        AccountMeta::new(tick_array_0, false),
        AccountMeta::new(tick_array_1, false),
        AccountMeta::new(tick_array_2, false),
        AccountMeta::new(oracle, false),
    ];

    let swap_data = SwapData {
        amount: swap_token_data.amount,
        other_amount_threshold: token_out_amount_min,
        sqrt_price_limit: 0,
        amount_specified_is_input: true,
        a_to_b,
    };

    let data = {
        let mut prefix = vec![248, 198, 158, 145, 225, 117, 135, 200];
        prefix.extend(swap_data.try_to_vec()?);
        prefix
    };

    Ok(Instruction {
        program_id: orca_swap_program_id,
        accounts,
        data,
    })


}


#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {

    dotenv().ok();



    let private_key = env::vars()
        .find(|(key, _)| key == "PRIVATE_KEY")
        .map(|(_, value)| value)
        .ok_or("PRIVATE_KEY not found")?;

    let user: Keypair = Keypair::from_base58_string(&private_key.as_str());
    println!("Initialized user wallet: {}", user.pubkey());

    let rpc_url = env::vars()
    .find(|(key, _)| key == "RPC_URL")
    .map(|(_, value)| value)
    .ok_or("RPC_URL not found")?;


    // 准备兑换数据

    const POOL_ADDRESS: &str = WSOL_USDC_3000;

    let amount = 3000_00; // token_in : usdc_amount
    let slippage = 10;


    let token_in = solana_sdk::pubkey::Pubkey::from_str(USDC_ADDRESS).unwrap();
    let token_out = solana_sdk::pubkey::Pubkey::from_str(WSOL_ADDRESS).unwrap();

    let pool_address = solana_sdk::pubkey::Pubkey::from_str(POOL_ADDRESS).unwrap();
    let dex_address = solana_sdk::pubkey::Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM_ID).unwrap();

    let swap_token_data = SwapTokenData::new(amount, token_in, token_out, USDC_DECIMALS, WSOL_DECIMALS, slippage);


    // 1. new 一个client


    let client = DEXClient::new(&rpc_url);
    println!("Connected to Solana mainnet");



    // 2. 用client拉取数据并且构建指令

    let instruction = build_orca_pool_swap_instruction(&client, &user, &swap_token_data, &pool_address, &dex_address, slippage).await.expect("build instruction error");
    println!(" build instruction finish ");


    // let token_a_balance_after = client.get_token_balance(&user_token_a_account).await?;
    // let token_b_balance_after = client.get_token_balance(&user_token_b_account).await?;

    // if a_to_b {
    //     println!("token_a_balance_after : {}", token_a_balance_before);
    //     println!("token_a_balance_after : {}", token_b_balance_before);
    // } else {
    //     println!("token_a_balance_after : {}", token_b_balance_after);
    //     println!("token_a_balance_after : {}", token_a_balance_after);
    // }




    // 3. 用client发送数据
    let _ = client.send_instructions( &user, vec![instruction]).await.expect("send instruction fail");
    println!(" success send instruction ");

    Ok(())
}
