# Build stage
FROM rust:1.74.1 as builder
WORKDIR /app
ADD . /app
RUN apt-get update
RUN apt install -y protobuf-compiler
RUN apt install -y ca-certificates
RUN cargo build --release

# Prod stage
FROM debian:bookworm
RUN apt-get update
RUN apt-get install -y ca-certificates
COPY --from=builder /app/target/release/elixr-task-service /
CMD ["./elixr-task-service"]