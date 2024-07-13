# Use a Debian base image
FROM ubuntu:latest as builder

# Install build dependencies and cross-compilers
RUN apt-get update && apt-get install -y \
    build-essential \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    make \
    wget \
    ca-certificates \
    perl \
    zip \
    unzip \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add cargo to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Add aarch64 target
RUN rustup target add aarch64-unknown-linux-gnu
RUN rustup default stable

## Install aws
RUN  curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip awscliv2.zip \
    && ./aws/install

# Check installation
RUN aws --version

# Set environment for cross-compilation
ENV OPENSSL_VERSION=openssl-3.3.1
ENV OPENSSL_DIR=/usr/local/openssl

# Download and extract OpenSSL
WORKDIR /tmp/${OPENSSL_VERSION}
RUN wget https://www.openssl.org/source/${OPENSSL_VERSION}.tar.gz \
    && tar -xzf ${OPENSSL_VERSION}.tar.gz \
    && cd ${OPENSSL_VERSION} \
    && ./Configure linux-aarch64 no-shared --prefix=${OPENSSL_DIR} --cross-compile-prefix=aarch64-linux-gnu- \
    && make \
    && make install
