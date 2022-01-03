# FROM clux/muslrust:stable as builder
# WORKDIR /usr/src/proxy
# COPY . .
# RUN cargo install --target=x86_64-unknown-linux-musl --path .

FROM alpine:latest
#COPY --from=builder /root/.cargo/bin/proxy /usr/local/bin/proxy
ADD ./encrypt /usr/local/bin/encrypt
CMD ["encrypt","-p","8888","-s","asia2.ethermine.org:4444"]