FROM rust:alpine AS builder

RUN apk update && apk add musl-dev
WORKDIR /workspace

COPY . .

RUN cargo install --path .

FROM alpine 
WORKDIR /app
RUN adduser -D app_user
COPY --from=builder --chown=app_user:app_user /workspace/target/release/staticshort . 
USER app_user
CMD [ "./staticshort" ]