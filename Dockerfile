FROM rust as builder
WORKDIR /usr/src/oxide_pinger
ADD . .

RUN cargo install --path .


FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/oxide_pinger /usr/local/bin/oxide_pinger
CMD ["oxide_pinger"]
