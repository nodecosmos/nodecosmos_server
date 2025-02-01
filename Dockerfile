# Stage 1: Builder
FROM ubuntu:24.10 AS builder

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
    git \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# Set up Rust environment
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Update PATH for subsequent RUN commands
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory
WORKDIR /usr/src/app

# Copy Cargo.toml and Cargo.lock to leverage Docker cache
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir -p nodecosmos/src
RUN mkdir -p macros/src

COPY nodecosmos/Cargo.toml nodecosmos/Cargo.toml
COPY macros/ macros/
COPY benchmarks/ benchmarks/

RUN echo "fn main() {}" > nodecosmos/src/main.rs

# Build dependencies
RUN cargo build --release
RUN rm -f target/release/deps/nodecosmos*

# Copy everything except the target directory
COPY . .

# Build the application
RUN cargo build --release

# Stage 2: Runtime
FROM ubuntu:22.04

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

# Set environment variables
ENV CONFIG_FILE=/etc/nodecosmos/config.toml

# Copy the configuration file
COPY config.development.toml /etc/nodecosmos/config.toml

# Expose the application port
EXPOSE 443

# Define the entrypoint
ENTRYPOINT ["./nodecosmos"]
