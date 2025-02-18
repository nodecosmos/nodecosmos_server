# Stage 1: Build dependencies
FROM ubuntu:24.10 AS builder

# Install system dependencies and Rust
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

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app

COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM ubuntu:24.10

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/nodecosmos .

# Define the entrypoint
ENTRYPOINT ["./nodecosmos"]
