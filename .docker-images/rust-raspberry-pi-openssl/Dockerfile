# This image is potentially useful to cross-compile rust applications for armv7 devices
# _with_ openssl also pre-compiled for that architecture. It is currently being used by
# the github workflow defined in the `.github` directory of this repo.
#
# build with:
# 
# docker build -t dadleyy/rust-raspberry-pi-openssl:latest .
FROM rust:1.60.0
LABEL homepage="https://github.com/dadleyy/orient-beetle/tree/master/.docker-images/rust-raspberry-pi-openssl"

RUN apt-get update
RUN apt-get install -y libssl-dev gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf
RUN rustup target add armv7-unknown-linux-gnueabihf
RUN rustup component add --target armv7-unknown-linux-gnueabihf rust-src rust-std rustc-dev
RUN cd /tmp && wget https://www.openssl.org/source/openssl-1.1.1g.tar.gz
RUN cd /tmp && tar xzf openssl-1.1.1g.tar.gz
RUN cd /tmp/openssl-1.1.1g && MACHINE=armv7 ARCH=arm CC=arm-linux-gnueabihf-gcc ./config shared
RUN cd /tmp/openssl-1.1.1g && MACHINE=armv7 ARCH=arm CC=arm-linux-gnueabihf-gcc make
