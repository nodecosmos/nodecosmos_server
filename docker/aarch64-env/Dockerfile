# Use Ubuntu as the base image
FROM ubuntu:22.04 as base

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    wget \
    ca-certificates \
    perl \
    zip \
    unzip \
    curl \
    openssl \
    libssl-dev \
    pkg-config \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# Set up Rust environment
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . $HOME/.cargo/env \
    && rustup default stable \
    && rustup target add aarch64-unknown-linux-gnu

# Use a multi-stage build to keep the final image clean and small
FROM base as builder

# Install cross-compilers
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# Environment variables for cross-compilation of OpenSSL
ENV AARCH64_OPENSSL_VERSION=openssl-3.0.8
ENV AARCH_OPENSSL_DIR=/usr/local/openssl-aarch64

# Download and build OpenSSL for aarch64
WORKDIR /tmp
RUN wget https://www.openssl.org/source/${AARCH64_OPENSSL_VERSION}.tar.gz \
    && tar -xzf ${AARCH64_OPENSSL_VERSION}.tar.gz \
    && cd ${AARCH64_OPENSSL_VERSION} \
    && ./Configure linux-aarch64 no-shared --prefix=${AARCH_OPENSSL_DIR} --cross-compile-prefix=aarch64-linux-gnu- \
    && make \
    && make install

# Final stage for running applications
FROM base as final

# Install AWS CLI
RUN curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip awscliv2.zip \
    && ./aws/install \
    && aws --version

# Copy necessary binaries and libraries from the builder stage
COPY --from=builder ${AARCH_OPENSSL_DIR} ${AARCH_OPENSSL_DIR}

# Set PATH to include Rust binaries
ENV PATH="/root/.cargo/bin:${PATH}"

# Install charybdis-migrate using cargo
RUN cargo install charybdis-migrate
