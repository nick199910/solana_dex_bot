extern crate dotenv;

use dotenv::dotenv;
use std::env;

mod utils;


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

const ORCA_SWAP_PROGRAM_ID: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const WSOL_ADDRESS: &str = "So11111111111111111111111111111111111111112";
const USDC_ADDRESS: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

pub struct OrcaSwapClient {
    rpc_client: AsyncRpcClient,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapData {
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,
}

impl OrcaSwapClient {
    pub fn new(rpc_url: &str) -> Self {
        let rpc_client = AsyncRpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        OrcaSwapClient { rpc_client }
    }

    pub async fn swap(
        &self,
        user: &Keypair,
        whirlpool: &Pubkey,
        token_program: &Pubkey,
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
        let orca_swap_program_id = Pubkey::from_str(ORCA_SWAP_PROGRAM_ID)?;
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

        let data = swap_data.try_to_vec()?;

        println!("======== {:?}", data);

        let instruction = Instruction {
            program_id: orca_swap_program_id,
            accounts,
            data: swap_data.try_to_vec()?,
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

    pub async fn get_or_create_associated_token_account(&self, user: &Keypair, mint: &Pubkey) -> std::result::Result<Pubkey, Box<dyn std::error::Error>> {
        let associated_token_address = get_associated_token_address(&user.pubkey(), mint);
        
        match self.rpc_client.get_account(&associated_token_address).await {
            Ok(_) => {
                println!("Associated token account already exists: {}", associated_token_address);
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

#[tokio::test]
async fn test_real_orca_swap() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let rpc_url = "https://api.mainnet-beta.solana.com/";
    let client = OrcaSwapClient::new(rpc_url);
    println!("Connected to Solana mainnet");

    dotenv().ok();

    let private_key = env::vars()
        .find(|(key, _)| key == "PRIVATE_KEY")
        .map(|(_, value)| value)
        .ok_or("PRIVATE_KEY not found")?;

    println!("private_key {}", private_key);
    let user = Keypair::from_base58_string(&private_key.as_str());

    println!("Initialized user wallet: {}", user.pubkey());

    let user_sol_account = client.get_or_create_associated_token_account(&user, &Pubkey::from_str(WSOL_ADDRESS)?).await?;
    let user_usdc_account = client.get_or_create_associated_token_account(&user, &Pubkey::from_str(USDC_ADDRESS)?).await?;

    // 这些地址需要根据实际情况进行调整
    let whirlpool = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")?;
    let token_program = spl_token::id();
    let token_vault_a = Pubkey::from_str("3YQm7ujtXWJU2e9jhp2QGHpnn1ShXn12QjvzMvDgabpX")?;
    let token_vault_b = Pubkey::from_str("2JTw1fE2wz1SymWUQ7UqpVtrTuKjcd6mWwYwUJUCh2rq")?;
    let tick_array_0 = Pubkey::from_str("CEstjhG1v4nUgvGDyFruYEbJ18X8XeN4sX1WFCLt4D5c")?;
    let tick_array_1 = Pubkey::from_str("A2W6hiA2nf16iqtbZt9vX8FJbiXjv3DBUG3DgTja61HT")?;
    let tick_array_2 = Pubkey::from_str("2Eh8HEeu45tCWxY6ruLLRN6VcTSD7bfshGj7bZA87Kne")?;
    let oracle = Pubkey::from_str("4GkRbcYg1VKsZropgai4dMf2Nj2PkXNLf43knFpavrSi")?;

    let sol_balance = client.get_token_balance(&user_sol_account).await?;
    let usdc_balance = client.get_token_balance(&user_usdc_account).await?;

    println!("Initial SOL balance: {}", sol_balance);
    println!("Initial USDC balance: {}", usdc_balance);

    let amount = 100_000; // 0.1 SOL (SOL has 9 decimals)
    let other_amount_threshold = 0; // 0.1 USDC (USDC has 6 decimals)
    let sqrt_price_limit = 0; // 0 means no limit
    let amount_specified_is_input = true;
    let a_to_b = true; // Swapping from SOL (A) to USDC (B)

    let signature = client.swap(
        &user,
        &whirlpool,
        &token_program,
        &user.pubkey(),
        &user_sol_account,
        &token_vault_a,
        &user_usdc_account,
        &token_vault_b,
        &tick_array_0,
        &tick_array_1,
        &tick_array_2,
        &oracle,
        amount,
        other_amount_threshold,
        sqrt_price_limit,
        amount_specified_is_input,
        a_to_b,
    ).await?;

    println!("Swap transaction signature: {}", signature);

    let sol_balance = client.get_token_balance(&user_sol_account).await?;
    let usdc_balance = client.get_token_balance(&user_usdc_account).await?;

    println!("SOL balance after swap: {}", sol_balance);
    println!("USDC balance after swap: {}", usdc_balance);

    Ok(())
}

fn main() {

    println!("Run 'cargo test' to execute the Orca Swap test.");
}
