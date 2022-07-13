FROM rust:1.62.0 as build

WORKDIR /build

RUN apt update -y && apt dist-upgrade -y
RUN rustup default nightly

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo build --release

FROM rust:1.62.0

WORKDIR /app

COPY --from=build /build/target/release/proxy /app/proxy

CMD [ "/app/proxy" ]
