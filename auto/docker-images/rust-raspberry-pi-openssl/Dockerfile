FROM rust:1.58.1

RUN apt-get update
RUN apt-get install -y libssl-dev gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf
RUN rustup target add armv7-unknown-linux-gnueabihf
RUN  rustup component add --target armv7-unknown-linux-gnueabihf rust-src rust-std rustc-dev
RUN cd /tmp && wget https://www.openssl.org/source/openssl-1.1.1g.tar.gz
RUN cd /tmp && tar xzf openssl-1.1.1g.tar.gz
RUN cd /tmp/openssl-1.1.1g && MACHINE=armv7 ARCH=arm CC=arm-linux-gnueabihf-gcc ./config shared
RUN cd /tmp/openssl-1.1.1g && MACHINE=armv7 ARCH=arm CC=arm-linux-gnueabihf-gcc make
