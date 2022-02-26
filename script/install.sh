#!/bin/bash
echo "-----------"
echo -n "输入web界面端口（如:8088）:"
read web_port
echo -n "输入web管理密码（最少8位数）:"
read web_pass
echo -n "输入web-Jwt秘钥(非常重要！！！！！32位以上随机数最好没有准好好随机数可以随便输入一些):"
read web_secret

rm -rf ~/proxy_tmp
cd ~/
mkdir ~/proxy_tmp
cd ~/proxy_tmp

wget -c "https://github.com/xbhuang1994/mining_proxy/releases/download/v0.2.2/mining_proxy.tar.gz"
tar -xf ./mining_proxy.tar.gz

rm -rf "/opt/MiningProxy/"
mkdir -p "/opt/MiningProxy/bin"
mkdir -p "/opt/MiningProxy/config"
mkdir -p "/opt/MiningProxy/logs"

mv mining_proxy "/opt/MiningProxy/bin"

cat > /opt/MiningProxy/config/MiningProxy.conf << EOF
MINING_PROXY_WEB_PORT=$web_port
MINING_PROXY_WEB_PASSWORD=$web_pass
JWT_SECRET=$web_secret
EOF

cat > /usr/lib/systemd/system/MiningProxy.service << EOF
[Unit]
Description=MiningProxy
After=network.target
After=network-online.target
Wants=network-online.target
[Service]
Type=simple
EnvironmentFile=/opt/MiningProxy/config/MiningProxy.conf
ExecStart=/opt/MiningProxy/bin/mining_proxy
ExecReload=/bin/kill -s HUP $MAINPID
ExecStop=/bin/kill -s QUIT $MAINPID
LimitNOFILE=65536
WorkingDirectory=/opt/MiningProxy/
Restart=always
[Install]
WantedBy=multi-user.target
EOF

rm -rf ~/proxy_tmp


systemctl daemon-reload
systemctl enable MiningProxy
systemctl start MiningProxy


echo "已经设置守护进程 MiningProxy"
echo "加入开机自启动 MiningProxy"

echo "MiningProxy 已经启动"
echo "启动命令: systemctl start MiningProxy"
echo "运行状态查看: systemctl status MiningProxy"
echo "停止命令: systemctl stop MiningProxy"
echo "重启命令(修改配置后执行此命令即可): systemctl restart MiningProxy"
echo "查看日志: journalctl -fu MiningProxy -o cat"
