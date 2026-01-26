# Glance development commands

# Compute worktree-specific container name and port
# Each worktree gets its own database instance to allow parallel development
_worktree_id := `basename "$PWD"`
_port_hash := `echo "$PWD" | cksum | cut -d' ' -f1`
export GLANCE_CONTAINER := "glance-postgres-" + _worktree_id
export GLANCE_DB_PORT := `echo $(( 5433 + $(echo "$PWD" | cksum | cut -d' ' -f1) % 100 ))`

# Start test database (unique per worktree)
db:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Using container: $GLANCE_CONTAINER on port $GLANCE_DB_PORT"
    # Check if container is already running and healthy
    if docker ps -q -f name="^${GLANCE_CONTAINER}$" -f health=healthy | grep -q .; then
        echo "Database already running"
    elif docker ps -q -f name="^${GLANCE_CONTAINER}$" | grep -q .; then
        echo "Container running, waiting for healthy..."
        until docker ps -q -f name="^${GLANCE_CONTAINER}$" -f health=healthy | grep -q .; do sleep 1; done
    else
        # Remove any stopped container with this name (e.g., from previous session)
        docker rm -f "$GLANCE_CONTAINER" 2>/dev/null || true
        docker compose up -d postgres
        echo "Waiting for database to be ready..."
        until docker ps -q -f name="^${GLANCE_CONTAINER}$" -f health=healthy | grep -q .; do sleep 1; done
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
    DATABASE_URL="postgres://glance:glance@localhost:${GLANCE_DB_PORT}/glance_test" cargo run

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
    echo "Resetting container: $GLANCE_CONTAINER"
    docker compose down -v
    docker compose up -d postgres
    echo "Waiting for database to be ready..."
    until docker ps -q -f name="^${GLANCE_CONTAINER}$" -f health=healthy | grep -q .; do sleep 1; done
