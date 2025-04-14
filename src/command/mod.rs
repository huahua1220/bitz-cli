mod account;
mod benchmark;
mod claim;
#[cfg(feature = "admin")]
mod initialize;
// mod collect; // 功能已移至 mine.rs
pub mod pool;
mod program;
mod stake;
mod transaction;
mod transfer;
mod mine;
mod stop_mining;
mod check;
mod miner_wrapper;
mod batch_mining; // 新增批量挖矿模块
