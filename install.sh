#!/bin/sh
# shellcheck shell=dash
#[[ $(id -u) != 0 ]] && echo -e "\n 请使用 ${red}root ${none}用户运行 ${yellow}~(^_^) ${none}\n" && exit 1
red='\e[91m'
green='\e[92m'
yellow='\e[94m'
magenta='\e[95m'
cyan='\e[96m'
none='\e[0m'
_red() { echo -e ${red}$*${none}; }
_green() { echo -e ${green}$*${none}; }
_yellow() { echo -e ${yellow}$*${none}; }
_magenta() { echo -e ${magenta}$*${none}; }
_cyan() { echo -e ${cyan}$*${none}; }


if [ "$KSH_VERSION" = 'Version JM 93t+ 2010-03-05' ]; then
    # The version of ksh93 that ships with many illumos systems does not
    # support the "local" extension.  Print a message rather than fail in
    # subtle ways later on:
    echo '请使用sh 执行 !' >&2
    exit 1
fi
# case $sys_bit in
# 'amd64' | x86_64) ;;
# *)
#     echo -e " 
# 	 这个 ${red}安装脚本${none} 不支持你的系统。 ${yellow}(-_-) ${none}
# 	备注: 仅支持 Ubuntu 16+ / Debian 8+ / CentOS 7+ 系统
# 	" && exit 1
#     ;;
# esac




set -u

set url = "https://github.com/dothinkdone/minerProxy/releases/download/v0.1.4/linux.tar.gz"

eth_miner_config_ask() {
    echo
    while :; do
        echo -e "是否开启 ETH抽水中转， 输入 [${magenta}Y/N${none}] 按回车"
        read -p "$(echo -e "(默认: [${cyan}Y${none}]):")" enableEthProxy
        [[ -z $enableEthProxy ]] && enableEthProxy="y"

        case $enableEthProxy in
        Y | y)
            enableEthProxy="y"
            eth_miner_config
            break
            ;;
        N | n)
            enableEthProxy="n"
            echo
            echo
            echo -e "$yellow 不启用ETH抽水中转 $none"
            echo "----------------------------------------------------------------"
            echo
            break
            ;;
        *)
            error
            ;;
        esac
    done
}

main() {
    eth_miner_config_ask 
  
    # mkdir -p ./opt/proxy/{bin,logs,config}
    # wget -c url
    # tar -xvf ./opt/linux.tar.gz
    # mv ./proxy ./opt/proxy/bin/
    # mv ./default.yaml ./opt/proxy/config/
    # mv ./identity.p12 ./opt/proxy/config/
}


gen_service() {
    touch proxy.service
    echo <<EOF
[Unit]
Description=Eth-Proxy
After=network.target
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/opt/proxy/bin/proxy -c /opt/proxy/config/default.yaml
ExecReload=/bin/kill -s HUP $MAINPID
ExecStop=/bin/kill -s QUIT $MAINPID
LimitNOFILE=65536
WorkingDirectory=/opt/proxy/
Restart=always
ReStartSec=600
[Install]
WantedBy=multi-user.target
EOF >> proxy.service;
}

need_cmd() {
    if ! check_cmd "$1"; then
        err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
}


main "$@" || exit 1