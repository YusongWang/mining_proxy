build : 
	cargo build  --release --target=x86_64-unknown-linux-musl
strip : 
	strip ./target/x86_64-unknown-linux-musl/release/proxy && strip ./target/x86_64-unknown-linux-musl/release/encrypt
upx : 
	upx --best --lzma ./target/x86_64-unknown-linux-musl/release/proxy && upx --best --lzma ./target/x86_64-unknown-linux-musl/release/encrypt
all : build strip upx

proxy : all
	docker build -t yusongwang/eth-proxy:v$(cat Cargo.toml | grep "version" | head -n 1 | sed 's/=/\n/g' | sed '1d' | sed 's/"/\n/g' | sed '1d' | sed '2d') .
encrypt : 
	docker build -t yusongwang/proxy-encrypt:v$(cat Cargo.toml | grep "version" | head -n 1 | sed 's/=/\n/g' | sed '1d' | sed 's/"/\n/g' | sed '1d' | sed '2d') .