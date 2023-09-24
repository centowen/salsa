FROM rust:1.67 as builder
RUN cargo install --locked trunk
RUN rustup target add wasm32-unknown-unknown
WORKDIR /usr/src/salsa-app
COPY . .
RUN cargo build
RUN cd /usr/src/salsa-app/frontend && mkdir -p /salsa/frontend && trunk build --public-url salsa/ --release -d /salsa/frontend
RUN cd /usr/src/salsa-app/backend && cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/backend /usr/local/bin/backend
COPY --from=builder /salsa/frontend /salsa/frontend
ENV RUST_LOG=Info
CMD ["backend", "--frontend-path", "/salsa/frontend", "--ip", "0.0.0.0"]
