# Stage 1: Install essential build tools and Rust
FROM ubuntu:24.10 AS base
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
    --no-install-recommends && \
    rm -rf /var/lib/apt/lists/*
# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /app

# Stage 2: Build Rust application
FROM base AS build
# Copy dependency files
COPY Cargo.toml Cargo.lock ./
COPY nodecosmos/Cargo.toml nodecosmos/Cargo.toml
COPY macros/ macros/
# Create a dummy main file so that Cargo can build a minimal binary
RUN mkdir -p nodecosmos/src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > nodecosmos/src/main.rs
# Build dependencies; this layer is cached unless Cargo.toml or related files change.
RUN cargo build --release --locked

# Copy the full source code.
COPY . .
RUN touch nodecosmos/src/main.rs

# Optionally, force file timestamps to update so Cargo detects changes:
# Build the final binary.
RUN cargo build --release --locked

# Stage 3: Create the runtime image
FROM ubuntu:24.10
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    libssl-dev && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
# Copy the built binary from the builder stage.
COPY --from=dependencies /app/target/release/nodecosmos .
ENTRYPOINT ["./nodecosmos"]
