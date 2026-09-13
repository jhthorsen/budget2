FROM rust:1-alpine AS build
WORKDIR /src
COPY . .
RUN apk add --no-cache build-base openssl-dev pkgconf sqlite-dev \
    && cargo build --release --locked --package budget2-web

FROM alpine
RUN apk add --no-cache ca-certificates libssl3 sqlite-libs \
    && adduser -D -h /app budget2
WORKDIR /app
COPY --from=build /src/target/release/budget2-web /usr/local/bin/budget2-web
RUN mkdir local && chown -R budget2:budget2 /app
USER budget2
ENV HOST=0.0.0.0
EXPOSE 3000
CMD ["budget2-web"]
