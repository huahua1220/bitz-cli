use clap::{Parser, Subcommand};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair, Signer},
};
use std::sync::Arc;

use args::*;
use error::Error;
use utils::{PoolCollectingData, SoloCollectingData};

mod args;
mod command;
mod error;
mod send;
mod utils;

// TODO: Unify balance and proof into "account"
// TODO: Move balance subcommands to "pool"
// TODO: Make checkpoint an admin command
// TODO: Remove boost command

#[derive(Clone)]
struct Miner {
    pub keypair_filepath: Option<String>,
    //私钥
    pub private_key: Option<String>,
    pub priority_fee: Option<u64>,
    pub dynamic_fee_url: Option<String>,
    pub dynamic_fee: bool,
    pub rpc_client: Arc<RpcClient>,
    pub fee_payer_filepath: Option<String>,
    pub fee_private_key: Option<String>,
    pub solo_collecting_data: Arc<std::sync::RwLock<Vec<SoloCollectingData>>>,
    pub pool_collecting_data: Arc<std::sync::RwLock<Vec<PoolCollectingData>>>,
    pub sub_private_filepath: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Fetch your account details")]
    Account(AccountArgs),

    #[command(about = "Benchmark your machine's hashpower")]
    Benchmark(BenchmarkArgs),

    #[command(about = "Claim your collecting yield")]
    Claim(ClaimArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Initialize the program")]
    Initialize(InitializeArgs),

    #[command(about = "Start collecting on your local machine")]
    Collect(CollectArgs),

    #[command(about = "Connect to a collecting pool")]
    Pool(PoolArgs),

    #[command(about = "Fetch onchain global program variables")]
    Program(ProgramArgs),

    #[command(about = "Manage your stake positions")]
    Stake(StakeArgs),

    #[command(about = "Fetch details about a BITZ transaction")]
    Transaction(TransactionArgs),

    #[command(about = "Send BITZ to another user")]
    Transfer(TransferArgs),
    
    #[command(about = "停止挖矿进程")]
    Stop(StopMiningArgs),
    
    #[command(about = "批量查询BITZ余额和挖矿时间")]
    Check(CheckArgs),
}

#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {
    #[arg(
        long,
        value_name = "NETWORK_URL",
        help = "Network address of your RPC provider",
        global = true
    )]
    rpc: Option<String>,

    #[clap(
        global = true,
        short = 'C',
        long = "config",
        id = "PATH",
        help = "Filepath to config file."
    )]
    config_file: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH",
        help = "Filepath to signer keypair.",
        global = true
    )]
    keypair: Option<String>,

    #[arg(
        long,
        value_name = "PRIVATE_KEY",
        help = "private key 私钥启动",
        global = true
    )]
    private_key: Option<String>,

    #[arg(
        long,
        value_name = "FEE_PAYER_FILEPATH",
        help = "Filepath to transaction fee payer keypair.",
        global = true
    )]
    fee_payer: Option<String>,

    #[arg(
        long,
        value_name = "FEE_PRIVATE_KEY",
        help = "start by fee payer private key 代支付gas私钥模式",
        global = true
    )]
    fee_private_key: Option<String>,

    #[arg(
        long,
        value_name = "MICROLAMPORTS",
        help = "Price to pay for compute units. If dynamic fees are enabled, this value will be used as the cap.",
        default_value = "1000",
        global = true
    )]
    priority_fee: Option<u64>,

    #[arg(
        long,
        value_name = "DYNAMIC_FEE_URL",
        help = "RPC URL to use for dynamic fee estimation.",
        global = true
    )]
    dynamic_fee_url: Option<String>,

    #[arg(long, help = "Enable dynamic priority fees", global = true)]
    dynamic_fee: bool,

    #[arg(
        long,
        value_name = "SUB_PRIVATE_FILEPATH",
        help = "JSON file containing private keys for batch mining",
        global = true
    )]
    sub_private: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    let args = Args::parse();

    // Load the config file from custom path, the default path, or use default config values
    let cli_config = if let Some(config_file) = &args.config_file {
        solana_cli_config::Config::load(config_file).unwrap_or_else(|_| {
            eprintln!("error: Could not find config file `{}`", config_file);
            std::process::exit(1);
        })
    } else if let Some(config_file) = &*solana_cli_config::CONFIG_FILE {
        solana_cli_config::Config::load(config_file).unwrap_or_default()
    } else {
        solana_cli_config::Config::default()
    };

    // 自定义默认RPC地址
    let default_rpc_url = String::from("https://eclipse.helius-rpc.com/");
    
    // Initialize miner.
    let cluster = args.rpc.unwrap_or_else(|| {
        if cli_config.json_rpc_url.is_empty() {
            default_rpc_url
        } else {
            cli_config.json_rpc_url
        }
    });
    
    let default_keypair = args.keypair.unwrap_or(cli_config.keypair_path.clone());
    let fee_payer_filepath = args.fee_payer.unwrap_or(default_keypair.clone());
    let rpc_client = RpcClient::new_with_commitment(cluster, CommitmentConfig::confirmed());

    let solo_collecting_data = Arc::new(std::sync::RwLock::new(Vec::new()));
    let pool_collecting_data = Arc::new(std::sync::RwLock::new(Vec::new()));

    let miner = Arc::new(Miner::new(
        Arc::new(rpc_client),
        args.priority_fee,
        Some(default_keypair),
        args.private_key,
        args.dynamic_fee_url,
        args.dynamic_fee,
        Some(fee_payer_filepath),
        args.fee_private_key,
        solo_collecting_data,
        pool_collecting_data,
        args.sub_private,
    ));

    // Execute user command.
    match args.command {
        Commands::Account(args) => {
            miner.account(args).await;
        }
        Commands::Benchmark(args) => {
            miner.benchmark(args).await;
        }
        Commands::Claim(args) => {
            if let Err(err) = miner.claim(args).await {
                println!("{:?}", err);
            }
        }
        Commands::Pool(args) => {
            miner.pool(args).await;
        }
        Commands::Program(_) => {
            miner.program().await;
        }
        Commands::Collect(args) => {
            if miner.sub_private_filepath.is_some() {
                if let Err(err) = miner.batch_collect(args).await {
                    println!("{:?}", err);
                }
            } else {
                if let Err(err) = miner.mine(args).await {
                    println!("{:?}", err);
                }
            }
        }
        Commands::Stake(args) => {
            miner.stake(args).await;
        }
        Commands::Transfer(args) => {
            miner.transfer(args).await;
        }
        Commands::Transaction(args) => {
            miner.transaction(args).await.unwrap();
        }
        #[cfg(feature = "admin")]
        Commands::Initialize(_) => {
            miner.initialize().await;
        }
        Commands::Stop(args) => {
            if let Err(err) = miner.terminate_mining(args) {
                println!("{:?}", err);
            }
        }
        Commands::Check(args) => {
            miner.check(args).await;
        }
    }
}

impl Miner {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        priority_fee: Option<u64>,
        keypair_filepath: Option<String>,
        private_key: Option<String>,
        dynamic_fee_url: Option<String>,
        dynamic_fee: bool,
        fee_payer_filepath: Option<String>,
        fee_private_key: Option<String>,
        solo_collecting_data: Arc<std::sync::RwLock<Vec<SoloCollectingData>>>,
        pool_collecting_data: Arc<std::sync::RwLock<Vec<PoolCollectingData>>>,
        sub_private_filepath: Option<String>,
    ) -> Self {
        Self {
            rpc_client,
            keypair_filepath,
            private_key,
            priority_fee,
            dynamic_fee_url,
            dynamic_fee,
            fee_payer_filepath,
            fee_private_key,
            solo_collecting_data,
            pool_collecting_data,
            sub_private_filepath,
        }
    }

    pub fn signer(&self) -> Keypair {
        if let Some(private_key) = &self.private_key {
            let bytes = bs58::decode(private_key)
                .into_vec()
                .expect("error private key，私钥格式错误");
            return Keypair::from_bytes(&bytes)
                .expect("从私钥启动失败");
        }
        
        match self.keypair_filepath.clone() {
            Some(filepath) => read_keypair_file(filepath.clone())
                .expect(format!("No keypair found at {}", filepath).as_str()),
            None => panic!("No keypair provided"),
        }
    }

    pub fn fee_payer(&self) -> Keypair {
        if let Some(fee_private_key) = &self.fee_private_key {
            let bytes = bs58::decode(fee_private_key)
                .into_vec()
                .expect("无效的fee payer私钥格式");
            return Keypair::from_bytes(&bytes)
                .expect("从fee payer私钥创建密钥对失败");
        }
        
        match self.fee_payer_filepath.clone() {
            Some(filepath) => read_keypair_file(filepath.clone())
                .expect(format!("No fee payer keypair found at {}", filepath).as_str()),
            None => panic!("No fee payer keypair provided"),
        }
    }
} 