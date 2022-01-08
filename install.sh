#!/bin/bash
echo -n "输入矿工名:"
read workname
echo -n "输入TCP端口:"
read tcp_port
echo -n "输入SSL端口:"
read ssl_port
echo -n "输入代理池TCP地址:"
read pool_tcp_address
echo -n "输入代理池SSL地址:"
read pool_ssl_address
echo -n "是否抽水? 0不抽水 1抽水:"
read share
echo -n "输入抽水池TCP地址:"
read share_tcp_address
echo -n "输入抽水钱包地址(0x开头):"
read share_wallet
echo -n "输入比例(1为百分之百0.01为百分之1):"
read share_rate
echo "-----------"
echo "Welcome,$workname!"
rm -rf ~/proxy_tmp
cd ~/
mkdir ~/proxy_tmp
cd ~/proxy_tmp

wget -c "https://github.com/dothinkdone/minerProxy/releases/download/v0.1.9/mining_proxy.tar.gz"
tar -xf ./mining_proxy.tar.gz

rm -rf "/opt/$workname/"
mkdir -p "/opt/$workname/bin"
mkdir -p "/opt/$workname/config"
mkdir -p "/opt/$workname/logs"

mv proxy "/opt/$workname/bin"
mv identity.p12 "/opt/$workname/config/"

cat > /opt/$workname/config/$workname.conf << EOF
PROXY_NAME="$workname"
PROXY_LOG_LEVEL=1
PROXY_LOG_PATH=""
PROXY_TCP_PORT=$tcp_port
PROXY_SSL_PORT=$ssl_port
PROXY_ENCRYPT_PORT=0
PROXY_POOL_SSL_ADDRESS="$pool_ssl_address"
PROXY_POOL_TCP_ADDRESS="$pool_tcp_address"
PROXY_SHARE_TCP_ADDRESS="$share_tcp_address"
PROXY_SHARE_SSL_ADDRESS=""
PROXY_SHARE_WALLET="$share_wallet"
PROXY_SHARE_RATE=$share_rate
PROXY_SHARE_NAME="$workname"
PROXY_SHARE=$share
PROXY_P12_PATH="/opt/$workname/config/identity.p12"
PROXY_P12_PASS="mypass"
PROXY_SHARE_ALG=1
PROXY_KEY="523B607044E6BF7E46AF75233FDC1278B7AA0FC42D085DEA64AE484AD7FB3664"
PROXY_IV="275E2015B9E5CA4DDB87B90EBC897F8C"
EOF

cat > /usr/lib/systemd/system/$workname.service << EOF
[Unit]
Description=Proxy
After=network.target
After=network-online.target
Wants=network-online.target
[Service]
Type=simple
EnvironmentFile=/opt/$workname/config/$workname.conf
ExecStart=/opt/$workname/bin/proxy
ExecReload=/bin/kill -s HUP $MAINPID
ExecStop=/bin/kill -s QUIT $MAINPID
LimitNOFILE=65536
WorkingDirectory=/opt/$workname/
Restart=always
[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable $workname
systemctl start $workname
echo "已经设置守护进程 $workname"
echo "加入开机自启动 $workname"

echo "$workname 已经启动"
echo "启动命令: systemctl start $workname"
echo "停止命令: systemctl stop $workname"
echo "重启命令(修改配置后执行此命令即可): systemctl restart $workname"
echo "查看日志: journalctl -fu $workname"