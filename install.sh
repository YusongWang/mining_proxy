#!/bin/sh
# shellcheck shell=dash

if [ "$KSH_VERSION" = 'Version JM 93t+ 2010-03-05' ]; then
    # The version of ksh93 that ships with many illumos systems does not
    # support the "local" extension.  Print a message rather than fail in
    # subtle ways later on:
    echo '请使用sh 执行 !' >&2
    exit 1
fi


set -u

set url = "https://github.com/dothinkdone/minerProxy/releases/download/v0.1.4/linux.tar.gz"

usage() {
    cat 1>&2 <<EOF
eth-proxy install scripnt version  0.0.1
install eth-proxy
EOF
}

get_version() {
    echo -n "Enter your name:"
    read name
    echo "hello $name,welcome to my program"     //显示信息
    exit 0
}


main() {
    usage()
    get_version()
    
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd wget
    need_cmd tar
    mkdir -p ./opt/proxy/{bin,logs,config}
    wget -c url
    tar -xvf ./opt/linux.tar.gz
    mv ./proxy ./opt/proxy/bin/
    mv ./default.yaml ./opt/proxy/config/
    mv ./identity.p12 ./opt/proxy/config/
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