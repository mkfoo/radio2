# base pre-built cross image
FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest

# add our foreign architecture and install our dependencies
RUN apt-get update && apt-get install -y --no-install-recommends apt-utils
RUN dpkg --add-architecture arm64
RUN apt-get update && apt-get -y install libasound2-dev:arm64

# add our linker search paths and link arguments
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-L /usr/lib/aarch64-linux-gnu -C link-args=-Wl,-rpath-link,/usr/lib/aarch64-linux-gnu $CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS"
