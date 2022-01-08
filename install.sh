#!/bin/bash
echo -n "输入矿工名:"
read workname
echo -n "输入代理池TCP地址:"
read pool_tcp_address
echo -n "输入代理池SSL地址:"
read pool_ssl_address
echo -n "是否抽水? 0不抽水 1抽水:"
read share
echo -n "输入抽水池SSL地址:"
read share_ssl_address
echo -n "输入抽水钱包地址(0x开头):"
read share_wallet
echo "-----------"
echo "Welcome,$workname!"
rm -rf ~/proxy_tmp
cd ~/
mkdir ~/proxy_tmp
cd ~/proxy_tmp
wget -c "https://github.com/dothinkdone/minerProxy/releases/download/v0.1.8/proxy.tar.gz"
tar -xvf ./proxy.tar.gz

rm -rf "/opt/$workname/"
mkdir -p "/opt/$workname/bin"
mkdir -p "/opt/$workname/config"
mkdir -p "/opt/$workname/logs"

mv proxy "/opt/$workname/bin"
mv identity.p12 "/opt/$workname/config/"
#mv default.yaml "/opt/$workname/config/"


cat > /opt/$workname/config/$workname.conf << EOF
PROXY_NAME="$workname"
PROXY_LOG_LEVEL=1
PROXY_LOG_PATH="/opt/$workname/logs/"
PROXY_TCP_PORT=8800
PROXY_SSL_PORT=14443
PROXY_POOL_SSL_ADDRESS="asia2.ethermine.org:5555"
PROXY_POOL_TCP_ADDRESS="asia2.ethermine.org:14444"
PROXY_SHARE_TCP_ADDRESS="asia2.ethermine.org:14444"
PROXY_SHARE_SSL_ADDRESS="asia2.ethermine.org:14444"
PROXY_SHARE_WALLET="$share_wallet"
PROXY_SHARE_RATE=0.01
PROXY_SHARE_NAME="$workname"
PROXY_SHARE=$share
PROXY_P12_PATH="/var/p12/identity.p12"
PROXY_P12_PASS="mypass"
PROXY_SHARE_ALG=1
PROXY_KEY: "523B607044E6BF7E46AF75233FDC1278B7AA0FC42D085DEA64AE484AD7FB3664"
PROXY_IV: "275E2015B9E5CA4DDB87B90EBC897F8C"
EOF


cat > /usr/lib/systemd/system/proxy.service << EOF
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