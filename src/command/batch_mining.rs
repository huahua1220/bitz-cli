use std::sync::Arc;
use std::io::Write;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_rpc_client::rpc_client::RpcClient;

use crate::{
    args::CollectArgs,
    error::Error,
    Miner,
};

impl Miner {
    pub async fn batch_collect(&self, args: CollectArgs) -> Result<(), Error> {
        if let Some(filepath) = &self.sub_private_filepath {
            // 读取 JSON 文件，获取私钥列表
            let file_content = std::fs::read_to_string(filepath)
                .expect("无法读取批量挖矿私钥文件");
            let private_keys: Vec<String> = serde_json::from_str(&file_content)
                .expect("JSON 文件格式不正确，应该是私钥字符串数组");

            if private_keys.is_empty() {
                println!("批量挖矿私钥文件为空，没有可用的私钥");
                return Ok(());
            }

            println!("开始批量挖矿，共 {} 个账户", private_keys.len());

            // 为每个私钥创建一个挖矿任务，但不使用tokio::spawn，避免Send问题
            for (idx, private_key) in private_keys.iter().enumerate() {
                // 为每个私钥创建独立的配置和客户端
                let _rpc_client = Arc::new(RpcClient::new_with_commitment(
                    self.rpc_client.url().to_string(),
                    CommitmentConfig::confirmed(),
                ));
                let priority_fee = self.priority_fee;
                let dynamic_fee_url = self.dynamic_fee_url.clone();
                let dynamic_fee = self.dynamic_fee;
                let _fee_payer_filepath = self.fee_payer_filepath.clone();
                let fee_private_key = self.fee_private_key.clone();
                
                println!("启动账户 #{} 挖矿任务: {}", idx + 1, private_key);
                
                // 创建一个新的独立进程来运行挖矿
                let pool_url = args.pool_url.clone();
                let args_cores = args.cores.clone();
                let args_buffer_time = args.buffer_time;
                let args_device_id = args.device_id;
                let args_verbose = args.verbose;
                
                // 使用std::process::Command来启动新进程，避免异步问题
                let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
                
                // 添加通用参数
                cmd.arg("collect")
                   .arg("--cores").arg(&args_cores)
                   .arg("--buffer-time").arg(args_buffer_time.to_string())
                   .arg("--private-key").arg(private_key);
                
                // 添加RPC URL
                cmd.arg("--rpc").arg(self.rpc_client.url());
                
                // 添加矿池URL（如果有）
                if let Some(url) = pool_url {
                    cmd.arg("--pool-url").arg(url);
                    if let Some(device_id) = args_device_id {
                        cmd.arg("--device-id").arg(device_id.to_string());
                    }
                }
                
                // 添加priority fee
                if let Some(fee) = priority_fee {
                    cmd.arg("--priority-fee").arg(fee.to_string());
                }
                
                // 添加fee payer私钥（如果有）
                if let Some(fee_key) = &fee_private_key {
                    cmd.arg("--fee-private-key").arg(fee_key);
                }
                
                // 添加verbose模式
                if args_verbose {
                    cmd.arg("--verbose");
                }
                
                // 添加dynamic fee相关参数
                if dynamic_fee {
                    cmd.arg("--dynamic-fee");
                    if let Some(url) = &dynamic_fee_url {
                        cmd.arg("--dynamic-fee-url").arg(url);
                    }
                }
                
                // 为每个进程创建日志文件
                let log_dir = std::path::Path::new("logs");
                if !log_dir.exists() {
                    std::fs::create_dir_all(log_dir).expect("无法创建日志目录");
                }
                
                let log_filename = format!("logs/miner_{}.log", idx + 1);
                let log_path = std::path::Path::new(&log_filename);
                let log_file = std::fs::File::create(log_path).expect("无法创建日志文件");
                
                // 将进程的输出重定向到日志文件
                cmd.stdout(log_file.try_clone().expect("无法复制文件句柄"))
                   .stderr(log_file);
                
                // 启动进程
                match cmd.spawn() {
                    Ok(child) => {
                        let pid = child.id();
                        println!("账户 #{} 挖矿进程已启动，进程ID: {}，日志文件: {}", idx + 1, pid, log_filename);
                        
                        // 保持子进程运行而不等待它，避免阻塞主进程
                        std::mem::forget(child);
                    },
                    Err(e) => {
                        println!("账户 #{} 启动挖矿进程失败: {:?}", idx + 1, e);
                    }
                }
            }

            // 创建一个进程列表文件以便于管理
            let process_list_path = "logs/process_list.txt";
            let mut process_list_file = std::fs::File::create(process_list_path).expect("无法创建进程列表文件");
            writeln!(process_list_file, "批量挖矿进程列表:").expect("写入文件失败");
            for (idx, _) in private_keys.iter().enumerate() {
                writeln!(process_list_file, "账户 #{}: 日志文件 logs/miner_{}.log", idx + 1, idx + 1).expect("写入文件失败");
            }

            println!("所有批量挖矿进程已启动，日志保存在logs目录下");
            println!("您可以通过查看logs/miner_*.log文件来监控挖矿状态");
            println!("进程列表已保存到{}", process_list_path);
            Ok(())
        } else {
            Err(Error::Internal("未指定批量挖矿私钥文件".to_string()))
        }
    }
} 