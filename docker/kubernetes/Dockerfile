# Stage 1: Build dependencies
FROM ubuntu:24.10 AS deps

# Install system dependencies and Rust
RUN apt-get update && apt-get install -y \
    build-essential ca-certificates curl openssl libssl-dev pkg-config --no-install-recommends && \
    rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY nodecosmos/Cargo.toml nodecosmos/Cargo.toml
COPY macros/ macros/

# Create dummy main files to satisfy Cargo in each crate as needed:
RUN mkdir -p nodecosmos/src && echo "fn main() {}" > nodecosmos/src/main.rs

# Build dependencies (this layer will be cached if the manifests don't change)
RUN cargo build --release

# Stage 2: Build the full application
FROM deps AS builder
# Now copy the rest of the source code (this invalidates the cache for later layers but keeps the deps layer cached)
COPY . .
RUN cargo build --release

# Stage 3: Final runtime image
FROM ubuntu:24.10
RUN apt-get update && apt-get install -y ca-certificates openssl libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /usr/src/app/target/release/nodecosmos .

# Define the entrypoint
ENTRYPOINT ["./nodecosmos"]
