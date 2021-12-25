build : 
	cargo +nightly build  --release --target=x86_64-unknown-linux-musl --out-dir=./ -Z unstable-options,build-std=std,panic_abort,build-std-features=panic_immediate_abort
strip : 
	strip ./proxy
upx : 
	upx --best --lzma ./proxy
all : build strip upx
