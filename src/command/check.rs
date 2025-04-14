use colored::*;
use solana_sdk::signature::{Keypair, Signer};
use tabled::{
    settings::{Alignment, Style},
    Table, Tabled,
};

use crate::{args::CheckArgs, Miner};
use crate::utils::{amount_u64_to_f64, format_timestamp, get_proof_with_authority};

#[derive(Tabled)]
struct AccountData {
    #[tabled(rename = "账户地址")]
    address: String,
    #[tabled(rename = "BITZ余额")]
    balance: String,
    #[tabled(rename = "上次挖矿时间")]
    last_hash_at: String,
}

impl Miner {
    pub async fn check(&self, args: CheckArgs) {
        let filepath = if let Some(path) = args.sub_private {
            path
        } else if let Some(path) = &self.sub_private_filepath {
            path.clone()
        } else {
            println!("{}", "错误: 未指定批量查询私钥文件。请使用 --sub-private 参数指定JSON私钥文件。".red());
            return;
        };

        // 读取JSON文件，获取私钥列表
        let file_content = match std::fs::read_to_string(&filepath) {
            Ok(content) => content,
            Err(e) => {
                println!("{}", format!("错误: 无法读取私钥文件 '{}': {}", filepath, e).red());
                return;
            }
        };

        let private_keys: Vec<String> = match serde_json::from_str(&file_content) {
            Ok(keys) => keys,
            Err(e) => {
                println!("{}", format!("错误: JSON文件格式不正确: {}", e).red());
                return;
            }
        };

        if private_keys.is_empty() {
            println!("{}", "警告: 私钥文件中没有找到任何私钥。".yellow());
            return;
        }

        println!("正在查询 {} 个账户的余额和挖矿时间...", private_keys.len());

        let mut account_data = Vec::new();

        // 查询每个账户的信息
        for private_key in private_keys {
            let bytes = match bs58::decode(&private_key).into_vec() {
                Ok(b) => b,
                Err(_) => {
                    println!("{}", format!("错误: 私钥格式错误: {}", private_key).red());
                    continue;
                }
            };

            let keypair = match Keypair::from_bytes(&bytes) {
                Ok(k) => k,
                Err(_) => {
                    println!("{}", "错误: 无法从私钥创建密钥对".red());
                    continue;
                }
            };

            let pubkey = keypair.pubkey();
            let address = pubkey.to_string();

            // 获取账户证明数据
            match get_proof_with_authority(&self.rpc_client, pubkey).await {
                Ok(proof) => {
                    let balance = format!("{:.10} BITZ", amount_u64_to_f64(proof.balance));
                    let last_hash_at = if proof.last_hash_at > 0 {
                        format_timestamp(proof.last_hash_at)
                    } else {
                        "从未挖矿".to_string()
                    };

                    account_data.push(AccountData {
                        address: format!("{}...{}", &address[0..4], &address[address.len() - 4..]),
                        balance,
                        last_hash_at,
                    });
                }
                Err(e) => {
                    account_data.push(AccountData {
                        address: format!("{}...{}", &address[0..4], &address[address.len() - 4..]),
                        balance: "无法获取".to_string(),
                        last_hash_at: "无法获取".to_string(),
                    });
                    println!("{}", format!("警告: 账户 {} 查询失败: {:?}", address, e).yellow());
                }
            }
        }

        // 显示表格
        if account_data.is_empty() {
            println!("{}", "未能成功查询任何账户信息。".red());
            return;
        }

        let mut table = Table::new(account_data);
        table
            .with(Style::modern())
            .with(Alignment::center());

        println!("{}", table);
    }
} 