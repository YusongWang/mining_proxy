build : 
	cargo build  --release --target=x86_64-unknown-linux-musl
ag_build : 
	cargo build  --features agent --release --target=x86_64-unknown-linux-musl
strip : 
	strip ./target/x86_64-unknown-linux-musl/release/proxy && strip ./target/x86_64-unknown-linux-musl/release/encrypt
upx : 
	upx --best --lzma ./target/x86_64-unknown-linux-musl/release/proxy && upx --best --lzma ./target/x86_64-unknown-linux-musl/release/encrypt
mv : 
	mv ./target/x86_64-unknown-linux-musl/release/proxy ./release/proxy && mv ./target/x86_64-unknown-linux-musl/release/encrypt ./release/encrypt
all : build strip upx mv

agent: ag_build strip upx mv

proxy :
	docker build -t yusongwang/eth-proxy:v$(cat Cargo.toml | grep "version" | head -n 1 | sed 's/=/\n/g' | sed '1d' | sed 's/"/\n/g' | sed '1d' | sed '2d') ./release/proxy/
encrypt : 
	docker build -t yusongwang/proxy-encrypt:v$(cat Cargo.toml | grep "version" | head -n 1 | sed 's/=/\n/g' | sed '1d' | sed 's/"/\n/g' | sed '1d' | sed '2d') ./release/encrypt/