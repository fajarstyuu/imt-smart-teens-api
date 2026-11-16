FROM rust:1.100.0-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS build
WORKDIR /build
ENV CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_DEBUG=0
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/build/target     rustc --version && cargo build --release --locked && cp target/release/imt-smart-teens-backend /tmp/imt-smart-teens

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl     && rm -rf /var/lib/apt/lists/*     && groupadd --gid 10001 imt && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin imt
COPY --from=build /tmp/imt-smart-teens /usr/local/bin/imt-smart-teens
COPY --chmod=0555 deploy/backend-entrypoint.sh /usr/local/bin/imt-entrypoint
USER 10001:10001
EXPOSE 4180
ENTRYPOINT ["/usr/local/bin/imt-entrypoint"]
CMD ["serve"]
