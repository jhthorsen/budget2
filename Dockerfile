FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN apt-get update \
    && apt-get install --yes --no-install-recommends libsqlite3-dev libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && cargo build --release --locked --package budget2-web

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates libsqlite3-0 libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && adduser --system --home /app --group budget2
WORKDIR /app
COPY --from=build /src/target/release/budget2-web /usr/local/bin/budget2-web
RUN mkdir local && chown -R budget2:budget2 /app
USER budget2
ENV HOST=0.0.0.0
ENV DATABASE_URL=sqlite:local/budget2.db
EXPOSE 3000
CMD ["budget2-web"]
