# Glance development commands

# Start test database (shared across worktrees)
db:
    #!/usr/bin/env bash
    set -euo pipefail
    # Check if glance-postgres container is already running and healthy
    if docker ps -q -f name=^glance-postgres$ -f health=healthy | grep -q .; then
        echo "Database already running"
    elif docker ps -q -f name=^glance-postgres$ | grep -q .; then
        echo "Container running, waiting for healthy..."
        until docker ps -q -f name=^glance-postgres$ -f health=healthy | grep -q .; do sleep 1; done
    else
        docker compose up -d postgres
        echo "Waiting for database to be ready..."
        until docker ps -q -f name=^glance-postgres$ -f health=healthy | grep -q .; do sleep 1; done
    fi

# Stop test database
db-down:
    docker compose down

# Run all tests
test: db
    cargo nextest run

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

# Quick check (no tests)
quick:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

# Run all CI checks locally (precommit hook)
precommit: db quick
    cargo nextest run

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
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose down -v
    docker compose up -d postgres
    echo "Waiting for database to be ready..."
    until docker ps -q -f name=^glance-postgres$ -f health=healthy | grep -q .; do sleep 1; done
