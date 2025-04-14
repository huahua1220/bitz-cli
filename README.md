取消了claim时候的Y/N确认

取消了挖矿地址需要最低0.0005ETH的限制，直接放空地址私钥挖，gas主地址出，领币直接领到主地址  

新增批量挖矿模式，仅支持私钥批量挖矿，不支持keypair

Windows编译  
安装rust环境和vc环境，然后下载代码,同目录下启动cmd输入命令
`cargo build --release`

打包完成后在target/release目录下启动cmd，在cmd里输入和linux一样的指令就能运行  
build报错的话看看缺什么环境，https://strawberryperl.com/  下个这个，还不行再安这俩`choco install openssl pkgconfiglite`

windows直接使用（适用本地环境安装有困难的用户）

只需要下载release文件夹，直接在文件夹的cmd里启动，命令见下面


Linux编译(ubuntu22版，24版好像有点小问题，自测吧)
```
sudo apt-get update && sudo apt-get upgrade -y
sudo apt install screen curl nano build-essential  -y
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone https://github.com/huahua1220/bitz-cli.git
cd bitz-cli
cargo build --release
cd ..
sudo ln -s ~/bitz-cli/target/release/bitz /usr/local/bin/bitz
cp ~/bitz-cli/release/bitz.json ~/bitz.json
```
在用户主目录下会有个bitz.json文件，放私钥用的
```
nano ~/bitz.json
```
打开编辑，xterminal直接在主目录下打开bitz.json编辑  

然后再执行下面命令就好

新增指令：  

单独挖矿模式  
加了两个参数  
--private-key 私钥  
--fee-private-key 私钥  
可以直接使用私钥，代替--keypair id.json --fee-payer id.json使用
  
批量挖矿模式（仅支持私钥，不支持keypair）：  
需要修改bitz.json文件，把"私钥1(private key1)"等json数据替换为挖矿地址私钥  

启动之后会在同目录下生成log文件夹，里面是运行日志
  
批量挖矿：`bitz collect --sub-private bitz.json --fee-private-key 支付gas的私钥 --rpc https://eclipse.helius-rpc.com/`  
  
批量停止挖矿：`bitz stop -k`  
  
批量查询余额：`bitz check --sub-private bitz.json --rpc https://eclipse.helius-rpc.com`  
  
批量领取bitz到主地址：`bitz claim --sub-private bitz.json --to 主地址 --fee-private-key 支付gas地址私钥 --rpc https://eclipse.helius-rpc.com`  
  




