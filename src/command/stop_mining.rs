use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use colored::*;

use crate::{Error, Miner, StopMiningArgs};

impl Miner {
    pub fn terminate_mining(&self, args: StopMiningArgs) -> Result<(), Error> {
        let log_dir = Path::new("logs");
        let process_list_path = log_dir.join("process_list.txt");
        
        // 检查进程列表文件是否存在
        if !process_list_path.exists() {
            println!("{}", "未找到批量挖矿进程列表文件，可能没有正在运行的挖矿进程".red());
            return Ok(());
        }
        
        // Windows和Linux/Mac使用不同的命令来列出和终止进程
        let list_cmd = if cfg!(target_os = "windows") {
            "tasklist"
        } else {
            "ps -e"
        };
        
        // 运行进程列表命令
        let output = Command::new(if cfg!(target_os = "windows") { "cmd" } else { "sh" })
            .args(if cfg!(target_os = "windows") { 
                vec!["/C", list_cmd] 
            } else { 
                vec!["-c", list_cmd] 
            })
            .output()
            .expect("无法获取当前运行的进程列表");
        
        let process_list = String::from_utf8_lossy(&output.stdout);
        let bitz_executable = if cfg!(target_os = "windows") {
            "bitz.exe"
        } else {
            "bitz"
        };
        
        // 读取日志目录中的所有日志文件，查找进程ID
        let mut killed_count = 0;
        
        if args.kill_all {
            println!("正在终止所有bitz挖矿进程...");
            
            // 获取所有包含bitz的进程
            let mut pids = Vec::new();
            
            // 在Windows上使用tasklist | findstr bitz
            // 在Linux/Mac上使用ps -e | grep bitz
            let find_cmd = if cfg!(target_os = "windows") {
                format!("tasklist | findstr {}", bitz_executable)
            } else {
                format!("ps -e | grep {}", bitz_executable)
            };
            
            let output = Command::new(if cfg!(target_os = "windows") { "cmd" } else { "sh" })
                .args(if cfg!(target_os = "windows") { 
                    vec!["/C", &find_cmd] 
                } else { 
                    vec!["-c", &find_cmd] 
                })
                .output()
                .expect("无法搜索bitz进程");
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // 解析进程ID
            for line in output_str.lines() {
                if !line.contains(bitz_executable) {
                    continue;
                }
                
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    if cfg!(target_os = "windows") {
                        // Windows: 第二列是PID
                        if let Ok(pid) = parts[1].parse::<u32>() {
                            pids.push(pid);
                        }
                    } else {
                        // Linux/Mac: 第一列是PID
                        if let Ok(pid) = parts[0].parse::<u32>() {
                            pids.push(pid);
                        }
                    }
                }
            }
            
            // 终止找到的进程
            for pid in pids {
                let kill_cmd = if cfg!(target_os = "windows") {
                    format!("taskkill /F /PID {}", pid)
                } else {
                    format!("kill -9 {}", pid)
                };
                
                let status = Command::new(if cfg!(target_os = "windows") { "cmd" } else { "sh" })
                    .args(if cfg!(target_os = "windows") { 
                        vec!["/C", &kill_cmd] 
                    } else { 
                        vec!["-c", &kill_cmd] 
                    })
                    .status();
                
                match status {
                    Ok(_) => {
                        killed_count += 1;
                        println!("已终止进程 ID: {}", pid);
                    },
                    Err(e) => {
                        println!("无法终止进程 {}: {}", pid, e);
                    }
                }
            }
        } else {
            // 尝试从logs目录中提取进程ID并终止
            if let Ok(entries) = fs::read_dir(log_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") &&
                       path.file_name().and_then(|s| s.to_str()).map_or(false, |name| name.starts_with("miner_")) {
                        
                        // 从日志文件中提取进程ID
                        if let Ok(file) = fs::File::open(&path) {
                            let reader = io::BufReader::new(file);
                            for line in reader.lines().flatten() {
                                if line.contains("进程ID:") {
                                    let parts: Vec<&str> = line.split("进程ID:").collect();
                                    if parts.len() > 1 {
                                        let parts: Vec<&str> = parts[1].trim().split(',').collect();
                                        if let Ok(pid) = parts[0].trim().parse::<u32>() {
                                            // 检查进程是否仍在运行
                                            let running = process_list.contains(&format!("{}", pid));
                                            
                                            if running {
                                                // 终止进程
                                                let kill_cmd = if cfg!(target_os = "windows") {
                                                    format!("taskkill /F /PID {}", pid)
                                                } else {
                                                    format!("kill -9 {}", pid)
                                                };
                                                
                                                let status = Command::new(if cfg!(target_os = "windows") { "cmd" } else { "sh" })
                                                    .args(if cfg!(target_os = "windows") { 
                                                        vec!["/C", &kill_cmd] 
                                                    } else { 
                                                        vec!["-c", &kill_cmd] 
                                                    })
                                                    .status();
                                                
                                                match status {
                                                    Ok(_) => {
                                                        killed_count += 1;
                                                        println!("已终止进程 ID: {} (来自日志文件 {})", pid, path.display());
                                                    },
                                                    Err(e) => {
                                                        println!("无法终止进程 {}: {}", pid, e);
                                                    }
                                                }
                                            }
                                            
                                            // 只处理第一个找到的进程ID
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 创建一个标记文件表示挖矿已停止
        let stop_marker = log_dir.join("mining_stopped.txt");
        if let Ok(mut file) = fs::File::create(stop_marker) {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let _ = writeln!(file, "批量挖矿进程已于 {} 停止，共终止 {} 个进程", timestamp, killed_count);
        }
        
        if killed_count > 0 {
            println!("{}", format!("成功终止 {} 个批量挖矿进程", killed_count).green());
        } else {
            println!("{}", "未找到任何运行中的批量挖矿进程".yellow());
        }
        
        Ok(())
    }
} 