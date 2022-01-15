FROM rustembedded/cross:aarch64-unknown-linux-musl-0.2.1

COPY openssl.sh /
RUN bash /openssl.sh linux-aarch64 aarch64-linux-musl-

ENV OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \