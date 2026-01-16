# Glance development commands

# Start test database
db:
    docker compose up -d postgres

# Stop test database
db-down:
    docker compose down

# Run all tests
test: db
    cargo test

# Run tests with Ollama integration
test-integration: db
    OLLAMA_AVAILABLE=1 cargo test

# Format and lint
check:
    cargo fmt --check
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Run the application
run:
    cargo run

# Run with test database
dev: db
    DATABASE_URL="postgres://glance:glance@localhost:5432/glance_test" cargo run

# Build release binary
build:
    cargo build --release

# Clean build artifacts
clean:
    cargo clean

# Watch for changes and run tests
watch:
    cargo watch -x test

# Show database logs
db-logs:
    docker compose logs -f postgres

# Reset database (stop, remove volume, restart)
db-reset:
    docker compose down -v
    docker compose up -d postgres
