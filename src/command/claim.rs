use std::str::FromStr;

use colored::*;
use eore_api::consts::MINT_ADDRESS;
use solana_program::pubkey::Pubkey;
use solana_sdk::{signature::{Keypair, Signature, Signer}};
use spl_token::amount_to_ui_amount;
use tabled::{
    settings::{Alignment, Style},
    Table, Tabled,
};

use crate::{
    args::ClaimArgs,
    error::Error,
    utils::{amount_f64_to_u64, ask_confirm, get_proof_with_authority, ComputeBudget, format_timestamp, amount_u64_to_f64},
    Miner,
};

use super::pool::Pool;

#[derive(Tabled)]
struct ClaimData {
    #[tabled(rename = "账户地址")]
    address: String,
    #[tabled(rename = "领取数量")]
    amount: String,
    #[tabled(rename = "交易状态")]
    status: String,
}

impl Miner {
    pub async fn claim(&self, args: ClaimArgs) -> Result<(), crate::error::Error> {
        // 检查是否为批量领取
        if let Some(filepath) = args.sub_private.as_ref() {
            return Box::pin(self.batch_claim(filepath, args.clone())).await;
        }

        // 单个账户领取逻辑
        match args.pool_url {
            Some(ref pool_url) => {
                let pool = &Pool {
                    http_client: reqwest::Client::new(),
                    pool_url: pool_url.clone(),
                };
                let _ = self.claim_from_pool(args, pool).await?;
                Ok(())
            }
            None => {
                self.claim_from_proof(args).await;
                Ok(())
            }
        }
    }

    async fn batch_claim(&self, filepath: &str, args: ClaimArgs) -> Result<(), crate::error::Error> {
        // 读取JSON文件，获取私钥列表
        let file_content = match std::fs::read_to_string(filepath) {
            Ok(content) => content,
            Err(e) => {
                println!("{}", format!("错误: 无法读取私钥文件 '{}': {}", filepath, e).red());
                return Err(Error::Internal("无法读取私钥文件".to_string()));
            }
        };

        let private_keys: Vec<String> = match serde_json::from_str(&file_content) {
            Ok(keys) => keys,
            Err(e) => {
                println!("{}", format!("错误: JSON文件格式不正确: {}", e).red());
                return Err(Error::Internal("JSON文件格式不正确".to_string()));
            }
        };

        if private_keys.is_empty() {
            println!("{}", "警告: 私钥文件中没有找到任何私钥。".yellow());
            return Ok(());
        }

        println!("开始批量领取 {} 个账户的奖励...", private_keys.len());

        let mut claim_data = Vec::new();

        // 依次处理每个私钥
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
            let short_address = format!("{}...{}", &address[0..4], &address[address.len() - 4..]);

            // 获取账户证明数据
            match get_proof_with_authority(&self.rpc_client, pubkey).await {
                Ok(proof) => {
                    if proof.balance == 0 {
                        claim_data.push(ClaimData {
                            address: short_address,
                            amount: "0.0000000000 BITZ".to_string(),
                            status: "无可领取奖励".yellow().to_string(),
                        });
                        continue;
                    }

                    // 设置临时Miner对象用于此次领取
                    let temp_miner = Miner {
                        keypair_filepath: None,
                        private_key: Some(private_key.clone()),
                        priority_fee: self.priority_fee,
                        dynamic_fee_url: self.dynamic_fee_url.clone(),
                        dynamic_fee: self.dynamic_fee,
                        rpc_client: self.rpc_client.clone(),
                        fee_payer_filepath: self.fee_payer_filepath.clone(),
                        fee_private_key: self.fee_private_key.clone(),
                        solo_collecting_data: self.solo_collecting_data.clone(),
                        pool_collecting_data: self.pool_collecting_data.clone(),
                        sub_private_filepath: None,
                    };

                    // 创建ClaimArgs的副本，但不包含sub_private以避免递归批量领取
                    let temp_args = ClaimArgs {
                        amount: args.amount,
                        to: args.to.clone(),
                        pool_url: args.pool_url.clone(),
                        sub_private: None,
                    };

                    // 尝试领取奖励，添加30秒超时
                    let formatted_amount = format!("{:.10} BITZ", amount_u64_to_f64(proof.balance));
                    let short_address_clone = short_address.clone();
                    
                    match tokio::time::timeout(std::time::Duration::from_secs(30), temp_miner.claim(temp_args)).await {
                        Ok(result) => match result {
                            Ok(_) => {
                                claim_data.push(ClaimData {
                                    address: short_address,
                                    amount: formatted_amount,
                                    status: "领取成功".green().to_string(),
                                });
                            },
                            Err(e) => {
                                claim_data.push(ClaimData {
                                    address: short_address,
                                    amount: formatted_amount,
                                    status: format!("领取失败: {}", e).red().to_string(),
                                });
                            }
                        },
                        Err(_) => {
                            claim_data.push(ClaimData {
                                address: short_address,
                                amount: formatted_amount,
                                status: "领取超时".red().to_string(),
                            });
                            println!("{}", format!("账户 {} 领取超时，跳过", short_address_clone).yellow());
                        }
                    }
                }
                Err(e) => {
                    claim_data.push(ClaimData {
                        address: short_address,
                        amount: "无法获取".to_string(),
                        status: format!("查询失败: {:?}", e).red().to_string(),
                    });
                }
            }
        }

        // 显示结果表格
        if claim_data.is_empty() {
            println!("{}", "未能成功处理任何账户。".red());
            return Ok(());
        }

        let mut table = Table::new(claim_data);
        table
            .with(Style::modern())
            .with(Alignment::center());

        println!("{}", table);
        
        Ok(())
    }

    pub async fn claim_from_proof(&self, args: ClaimArgs) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let proof = get_proof_with_authority(&self.rpc_client, pubkey).await.expect("Failed to fetch proof account");
        let mut ixs = vec![];
        let beneficiary = match args.to {
            None => self.initialize_ata(pubkey).await,
            Some(to) => {
                // Create beneficiary token account, if needed
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let benefiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&benefiary_tokens)
                    .await
                    .is_err()
                {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &pubkey,
                            &wallet,
                            &eore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                benefiary_tokens
            }
        };

        // Parse amount to claim
        let amount = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            proof.balance
        };
        /* 
        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to claim {}.\n\nAre you sure you want to continue? [Y/n]",
                format!(
                    "{} BITZ",
                    amount_to_ui_amount(amount, eore_api::consts::TOKEN_DECIMALS)
                )
                .bold(),
            )
            .as_str(),
        ) {
            return;
        }
        */
        // Send and confirm
        ixs.push(eore_api::sdk::claim(pubkey, beneficiary, amount));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(32_000), false)
            .await
            .ok();
    }

    async fn claim_from_pool(
        &self,
        args: ClaimArgs,
        pool: &Pool,
    ) -> Result<Signature, crate::error::Error> {
        let pool_address = pool.get_pool_address().await?;
        let member = pool
            .get_pool_member_onchain(self, pool_address.address)
            .await?;
        let mut ixs = vec![];

        // Create beneficiary token account, if needed
        let beneficiary = match args.to {
            None => self.initialize_ata(self.signer().pubkey()).await,
            Some(to) => {
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let benefiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&benefiary_tokens)
                    .await
                    .is_err()
                {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &self.signer().pubkey(),
                            &wallet,
                            &eore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                benefiary_tokens
            }
        };

        // Parse amount to claim
        let amount = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            member.balance
        };
        /* 
        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to claim {}.\n\nAre you sure you want to continue? [Y/n]",
                format!(
                    "{} BITZ",
                    amount_to_ui_amount(amount, eore_api::consts::TOKEN_DECIMALS)
                )
                .bold(),
            )
            .as_str(),
        ) {
            return Err(crate::error::Error::Internal("exited claim".to_string()));
        }
        */
        // Send and confirm
        ixs.push(ore_pool_api::sdk::claim(
            self.signer().pubkey(),
            beneficiary,
            pool_address.address,
            amount,
        ));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(50_000), false)
            .await
            .map_err(From::from)
    }

    pub async fn initialize_ata(&self, wallet: Pubkey) -> Pubkey {
        // Initialize client.
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &wallet,
            &eore_api::consts::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }
        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &signer.pubkey(),
            &signer.pubkey(),
            &eore_api::consts::MINT_ADDRESS,
            &spl_token::id(),
        );
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(400_000), false)
            .await
            .ok();

        // Return token account address
        token_account_pubkey
    }
}
