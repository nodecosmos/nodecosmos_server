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
    && rustup default stable

# Use a multi-stage build to keep the final image clean and small
FROM base as builder

# Set PATH to include Rust binaries
ENV PATH="/root/.cargo/bin:${PATH}"

# Install charybdis-migrate using cargo
RUN cargo install charybdis-migrate

RUN mkdir /app
WORKDIR /app
RUN cargo build --release

# Runtime Stage
FROM base as runtime
WORKDIR /app
COPY --from=builder /app/target/release/nodecosmos .
CMD ["./nodecosmos"]
