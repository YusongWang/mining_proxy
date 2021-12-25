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


UPDATE_ROOT="${UPDATE_ROOT:-https://github.com/dothinkdone/minerProxy/releases/download/{$tag}}/linux.tar.gz}"


usage() {
    cat 1>&2 <<EOF
eth-proxy install scripnt version  0.0.1
install eth-proxy
EOF
}


main() {
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd wget
    need_cmd tar
    
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