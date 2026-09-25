FROM alpine:3.20

# Install SSL certificates for outbound HTTPS requests
RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy binary from host build context (pre-built on the runner)
COPY ./target/aarch64-unknown-linux-musl/release/rust-tmpl /app/rust-tmpl

# Rendered mdBook served at /docs (build it with `mdbook build` before the image)
COPY ./book /app/book

# Expose port
EXPOSE 80

# Set environment variables defaults
ENV PORT=80
ENV DATABASE_URL=sqlite:///app/database.db
ENV JWT_SECRET=super-secret-jwt-key
ENV BOOK_ROOT=/app/book

CMD ["/app/rust-tmpl"]
