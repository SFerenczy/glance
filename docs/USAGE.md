# User Guide

> How to use Glance to query your database with natural language

---

## Quick Start

### 1. Connect to Your Database

```bash
# Using a connection string
glance "postgres://user:password@localhost:5432/mydb"

# Using command-line arguments
glance --host localhost --database mydb --user postgres --password

# Using a saved connection (from config file)
glance --connection prod
```

### 2. Ask Questions

Type your question in natural language and press Enter:

```
> How many users signed up this month?
```

Glance will:

1. Send your question to the LLM
2. Generate appropriate SQL
3. Execute the query (or ask for confirmation if it modifies data)
4. Display results in a formatted table

### 3. View Results

Results appear as formatted tables in the chat panel:

```
┌────┬────────────────┬────────────┐
│ id │ email          │ name       │
├────┼────────────────┼────────────┤
│ 1  │ alice@test.com │ Alice      │
│ 2  │ bob@test.com   │ Bob        │
│ 3  │ carol@test.com │ NULL       │
└────┴────────────────┴────────────┘
3 rows returned (23ms)
```

---

## Interface Overview

```
┌─────────────────────────────────────────────────────────────┐
│ Glance v0.1                           [db: myapp @ localhost]│
├─────────────────────────────────────────┬───────────────────┤
│                                         │ Query Log         │
│  Chat Panel                             │                   │
│  (your conversation)                    │ ▸ SELECT * FRO... │
│                                         │   ✓ 23ms, 47 rows │
│                                         │                   │
│                                         │ ▸ SELECT COUNT... │
│                                         │   ✓ 8ms, 1 row    │
├─────────────────────────────────────────┴───────────────────┤
│ > _                                                         │
└─────────────────────────────────────────────────────────────┘
```

### Panels

- **Header**: Shows app version and connected database
- **Chat Panel**: Conversation history with questions, answers, and results
- **Query Log**: Sidebar showing executed SQL queries
- **Input Bar**: Where you type questions and commands

---

## Keyboard Shortcuts

| Key          | Action                          |
| ------------ | ------------------------------- |
| `Enter`      | Submit input                    |
| `Ctrl+C`     | Exit application                |
| `Ctrl+Q`     | Exit application                |
| `Ctrl+L`     | Clear chat history              |
| `Tab`        | Switch focus between panels     |
| `↑/↓`        | Scroll chat or navigate sidebar |
| `Page Up/Dn` | Scroll chat by page             |
| `Home/End`   | Scroll to top/bottom            |
| `Esc`        | Cancel operation / close modal  |

---

## Commands

Type these commands in the input bar:

| Command            | Description                        |
| ------------------ | ---------------------------------- |
| `/sql <query>`     | Execute raw SQL directly           |
| `/clear`           | Clear chat history and LLM context |
| `/schema`          | Display database schema summary    |
| `/refresh schema`  | Refresh database schema            |
| `/help`            | Show available commands            |
| `/quit` or `/exit` | Exit application                   |

### Connection Commands

| Command                      | Description                  |
| ---------------------------- | ---------------------------- |
| `/connections`               | List saved connections       |
| `/connect <name>`            | Switch to a saved connection |
| `/conn add <name> <params>`  | Add a new connection         |
| `/conn edit <name> <params>` | Edit an existing connection  |
| `/conn delete <name>`        | Delete a connection          |

**Connection parameters**: `backend=`, `host=`, `port=`, `database=`, `user=`, `password=`, `sslmode=`

Example:

```
/conn add mydb backend=postgres host=localhost port=5432 database=mydb user=postgres --test
```

### LLM Commands

| Command               | Description               |
| --------------------- | ------------------------- |
| `/llm`                | Show current LLM settings |
| `/llm provider`       | Show current provider     |
| `/llm provider <val>` | Set LLM provider          |
| `/llm model`          | Show current model        |
| `/llm model <val>`    | Set LLM model             |
| `/llm key`            | Show API key status       |
| `/llm key <val>`      | Set API key               |

### Query History Commands

| Command                | Description          |
| ---------------------- | -------------------- |
| `/history`             | Show query history   |
| `/history clear`       | Clear query history  |
| `/savequery <name>`    | Save last query      |
| `/queries`             | List saved queries   |
| `/usequery <name>`     | Load a saved query   |
| `/query delete <name>` | Delete a saved query |

### Examples

```
> /sql SELECT COUNT(*) FROM orders WHERE status = 'pending'

> /schema

> /clear
```

---

## Query Safety

Glance automatically classifies queries by safety level:

### Safe Queries (Auto-Execute)

These run immediately without confirmation:

- `SELECT` - Read data
- `EXPLAIN` - Query plans
- `SHOW` - Database settings

### Mutating Queries (Confirm)

These require confirmation before execution:

- `INSERT` - Add data
- `UPDATE` - Modify data
- `MERGE` - Upsert data

```
┌─────────────────────────────────────────────────────────┐
│ ⚠ This query will modify data:                         │
│                                                         │
│   UPDATE users SET status = 'inactive'                  │
│   WHERE last_login < '2024-01-01'                       │
│                                                         │
│ Execute? [y/Enter] Yes  [n/Esc] No                      │
└─────────────────────────────────────────────────────────┘
```

### Destructive Queries (Confirm + Warning)

These show a strong warning:

- `DELETE` - Remove data
- `DROP` - Remove objects
- `TRUNCATE` - Clear tables
- `ALTER` - Modify schema

```
┌─────────────────────────────────────────────────────────┐
│ 🛑 WARNING: This query may cause data loss:             │
│                                                         │
│   DELETE FROM orders WHERE status = 'cancelled'         │
│                                                         │
│ This action cannot be undone. Execute? [y/N]            │
└─────────────────────────────────────────────────────────┘
```

---

## Configuration

### Config File Location

- **Linux/macOS**: `~/.config/db-glance/config.toml`
- **Windows**: `%APPDATA%\db-glance\config.toml`

### Example Configuration

```toml
# LLM Configuration
[llm]
provider = "openai"           # "openai" or "anthropic"
model = "gpt-5"              # Model to use

# Default connection (used when no --connection specified)
[connections.default]
host = "localhost"
port = 5432
database = "mydb"
user = "postgres"
# password = "secret"         # Optional (will prompt if omitted)

# Additional named connections
[connections.prod]
host = "prod.example.com"
port = 5432
database = "production"
user = "readonly"
```

### Environment Variables

| Variable              | Description                              |
| --------------------- | ---------------------------------------- |
| `OPENAI_API_KEY`      | API key for OpenAI                       |
| `ANTHROPIC_API_KEY`   | API key for Anthropic                    |
| `GLANCE_LLM_PROVIDER` | Default LLM provider (openai, anthropic) |
| `OPENAI_MODEL`        | Default model for OpenAI                 |
| `ANTHROPIC_MODEL`     | Default model for Anthropic              |
| `GLANCE_DB_POOL_SIZE` | SQLite state DB pool size (default: 4)   |
| `PGHOST`              | Default PostgreSQL host                  |
| `PGPORT`              | Default PostgreSQL port                  |
| `PGDATABASE`          | Default database name                    |
| `PGUSER`              | Default database user                    |
| `PGPASSWORD`          | Default database password                |

---

## Connection Options

### Command-Line Arguments

```bash
glance [OPTIONS] [CONNECTION_STRING]

Arguments:
  [CONNECTION_STRING]  PostgreSQL connection string

Options:
  -h, --host <HOST>          Database host
  -p, --port <PORT>          Database port [default: 5432]
  -d, --database <DATABASE>  Database name
  -U, --user <USER>          Database user
  -W, --password             Prompt for password
  -c, --connection <NAME>    Use named connection from config
      --config <PATH>        Config file path
  -v, --version              Print version
      --help                 Print help
```

### Connection Priority

Arguments are resolved in this order (highest priority first):

1. Command-line arguments
2. Connection string
3. Named connection from config (`--connection`)
4. Default connection from config
5. Environment variables (`PGHOST`, etc.)

---

## Tips and Best Practices

### Effective Questions

**Good questions:**

- "Show me the top 10 customers by order value"
- "How many orders were placed last week?"
- "Which products have never been ordered?"

**Less effective:**

- "Show me everything" (too vague)
- "Do the thing" (unclear intent)

### Using Context

Glance maintains conversation context, so you can ask follow-up questions:

```
> Show me all users
[results displayed]

> Now filter to just those who signed up this year
[refined query executed]
```

### Raw SQL for Precision

When you know exactly what you need, use `/sql`:

```
> /sql SELECT u.name, COUNT(o.id) as order_count
       FROM users u
       LEFT JOIN orders o ON u.id = o.user_id
       GROUP BY u.id, u.name
       ORDER BY order_count DESC
       LIMIT 10
```

### Viewing Query History

Press `Tab` to focus the sidebar, then use arrow keys to browse executed queries. Press `Enter` to see the full SQL.

---

## Troubleshooting

### Connection Issues

**"Cannot connect to host:port"**

- Verify the database server is running
- Check host and port are correct
- Ensure firewall allows the connection

**"Authentication failed"**

- Verify username and password
- Check user has permission to connect

**"Database does not exist"**

- Verify database name spelling
- Ensure database exists on the server

### LLM Issues

**"API key missing"**

- Set `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` environment variable
- Or add to your shell profile (`.bashrc`, `.zshrc`)

**"Rate limited"**

- Wait a moment and try again
- Consider upgrading your API plan

### Query Issues

**"Column does not exist"**

- Check column name spelling
- Use `/schema` to see available columns

**"Query timed out"**

- Query took longer than 30 seconds
- Try adding LIMIT or more specific WHERE clauses

---

## Limitations (v0.2)

Current limitations that may be addressed in future versions:

- PostgreSQL only (MySQL, SQLite planned)
- Single database connection at a time
- No clipboard copy support
- No export to file
- No custom themes
- Limited mouse support (scroll only)

---

## LLM Configuration Resolution

LLM settings are resolved in this order (highest priority first):

1. **CLI arguments** (`--llm`, `--model`)
2. **Persisted settings** (via `/llm` commands)
3. **Environment variables** (`GLANCE_LLM_PROVIDER`, `OPENAI_MODEL`, etc.)
4. **Provider defaults**

This allows you to set defaults via environment variables, override them per-session with `/llm` commands, and override everything with CLI arguments for testing.
