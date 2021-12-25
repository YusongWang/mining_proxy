build : 
	cargo +nightly build  --release --target=x86_64-unknown-linux-musl --out-dir=./ -Z unstable-options
strip : 
	strip ./proxy
upx : 
	upx --best --lzma ./proxy
all : build strip upx

docker : all
	docker build -t yusongwang:eth-proxy:v{$version} . && yusongwang:eth-proxy:v{$version}