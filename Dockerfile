# -----------------------------
# Build Kdash base image
# -----------------------------

FROM clux/muslrust:stable AS builder
WORKDIR /usr/src

# Download and compile Rust dependencies in an empty project and cache as a separate Docker layer
RUN USER=root cargo new --bin kdash-temp

WORKDIR /usr/src/kdash-temp
COPY Cargo.* .
RUN cargo build --release --target x86_64-unknown-linux-musl
# remove src from empty project
RUN rm -r src
# Copy actual source files and Build the app binary
COPY src ./src
# remove previous deps
RUN rm ./target/x86_64-unknown-linux-musl/release/deps/kdash*

RUN --mount=type=cache,target=/volume/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release --target x86_64-unknown-linux-musl --bin kdash
RUN mv target/x86_64-unknown-linux-musl/release/kdash .

# -----------------------------
# build final Kdash image
# -----------------------------
FROM debian:stable-slim

ARG KUBECTL_VERSION="v1.29.0"
# Copy the compiled binary from the builder container
COPY --from=builder --chown=nonroot:nonroot /usr/src/kdash-temp/kdash /usr/local/bin

# Install dependencies like kubectl
RUN apt-get update && \
    apt-get dist-upgrade -y && \
    apt-get install -y -qq libxcb1 curl vim && \
    curl -L https://storage.googleapis.com/kubernetes-release/release/${KUBECTL_VERSION}/bin/linux/amd64/kubectl -o /usr/local/bin/kubectl && \
    chmod +x /usr/local/bin/kubectl && \
    apt-get autoremove && apt-get autoclean

RUN /usr/local/bin/kdash -h

ENTRYPOINT [ "/usr/local/bin/kdash" ]
