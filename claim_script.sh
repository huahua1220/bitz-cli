#!/bin/bash

# 固定参数
FEE_PAYER="/home/ubuntu/.config/solana/id.json"
TO_ADDRESS="bdDV51AhVq2x6qqv7pTzNMTD8Xs848ViuBUT8VAWHUA"
RPC_URL="https://api.mainnet-beta.solana.com"
COUNT=10 # 处理10个钱包

# 显示开始信息
echo "开始领取余额...处理 $COUNT 个钱包"

# 批量领取函数
claim_funds() {
    local keypair=$1
    local keypair_name=$(basename "$keypair" .json)
    
    echo "正在处理 $keypair_name..."
    # 延迟1秒
    sleep 1
    # 使用echo "Y" |来自动回复确认提示
    echo "Y" | bitz claim --keypair "$keypair" --fee-payer "$FEE_PAYER" --to "$TO_ADDRESS" --rpc "$RPC_URL"
}

# 动态创建钱包地址数组
KEYPAIRS=()
for ((i=1; i<=$COUNT; i++)); do
    KEYPAIRS+=("/home/ubuntu/.config/solana/id$i.json")
done

# 使用并行处理
for keypair in "${KEYPAIRS[@]}"; do
    if [ -f "$keypair" ]; then
        claim_funds "$keypair" &
    else
        echo "警告: $keypair 不存在，已跳过"
    fi
done

# 等待所有后台任务完成
wait

echo "领取完成" 