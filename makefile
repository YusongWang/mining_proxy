build : 
	cargo build  --release --target=x86_64-unknown-linux-musl
ag_build : 
	cargo build  --release --target=x86_64-unknown-linux-musl
strip : 
	strip ./target/x86_64-unknown-linux-musl/release/mining_proxy
upx : 
	upx --best --lzma ./target/x86_64-unknown-linux-musl/release/mining_proxy
mv : 
	mv ./target/x86_64-unknown-linux-musl/release/mining_proxy ./release/mining_proxy
all : build strip upx mv
