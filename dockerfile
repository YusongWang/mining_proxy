# FROM rustembedded/cross:aarch64-unknown-linux-musl-0.2.1

# COPY openssl.sh /
# RUN bash /openssl.sh linux-aarch64 aarch64-linux-musl-

# ENV OPENSSL_DIR=/openssl \
#     OPENSSL_INCLUDE_DIR=/openssl/include \
#     OPENSSL_LIB_DIR=/openssl/lib \

FROM messense/rust-musl-cross:armv7-musleabihf
RUN rustup update && \
    rustup update beta && \
    rustup update nightly && \
    rustup target add --toolchain beta armv7-unknown-linux-musleabihf && \
    rustup target add --toolchain nightly armv7-unknown-linux-musleabihf