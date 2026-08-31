FROM mcr.microsoft.com/devcontainers/typescript-node:1-22-bookworm

ARG RUST_VERSION=1.98.0
ARG NPM_VERSION=11.18.0
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/home/node/.cargo
ENV PATH="/home/node/.cargo/bin:${PATH}"
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 python3-pip protobuf-compiler \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /home/node/.cargo \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}" \
    && rustup component add rustfmt clippy \
    && rustup target add x86_64-pc-windows-msvc \
    && chown -R node:node /home/node/.cargo \
    && chmod -R a+rX /usr/local/rustup \
    && npm install --global "npm@${NPM_VERSION}" \
    && python3 -m pip install --break-system-packages pip-audit==2.10.1 \
    && cargo install cargo-audit --locked
