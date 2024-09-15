use solana_sdk::pubkey::Pubkey;
use std::{default, str::FromStr};

#[derive(Default)]
struct PdaUtils;

impl PdaUtils {
    fn new() -> PdaUtils {
        PdaUtils{  }
    }

    fn get_tick_array_pda(program_id: Pubkey, whirlpool_address: Pubkey, start_tick: i32) -> Pubkey {
        let (tick_array_pda, _bump_seed) = Pubkey::find_program_address(
            &[
                b"tick_array",
                &whirlpool_address.to_bytes(),
                &start_tick.to_le_bytes(),
            ],
            &program_id,
        );
        tick_array_pda
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use solana_client::rpc_client::RpcClient;
    use solana_sdk::pubkey::Pubkey;
    use crate::utils::PdaUtils;


    // #[test]
    fn test_get_tick_array_pda() {
        let rpc_url = "https://api.mainnet-beta.solana.com";
        let client = RpcClient::new(rpc_url.to_string());

        let program_id = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap();
        let whirlpool_address = Pubkey::from_str("2LecshUwdy9xi7meFgHtFJQNSKk4KdTrcpvaB56dP2NQ").unwrap();
        let start_tick: i32 = 0;
        let tick_array_pda = PdaUtils::get_tick_array_pda(program_id, whirlpool_address, start_tick);
        println!("TickArray PDA 地址: {:?}", tick_array_pda);
        match client.get_account(&tick_array_pda) {
            Ok(account_info) => {
                println!("账户数据: {:?}", account_info.data);
            }
            Err(err) => {
                eprintln!("获取账户信息失败: {:?}", err);
            }
        }
    }

}

