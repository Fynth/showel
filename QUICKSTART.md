# Showel Quick Start Guide

Get up and running with Showel in 5 minutes!

## Prerequisites

- Rust toolchain (1.70+)
- PostgreSQL server running

## Installation

```bash
# Clone and build
git clone <your-repo>
cd showel
cargo build --release

# Run
cargo run --release
```

## First Connection

1. **Launch Showel**
   - The application window will open
   - You'll see "Not connected" status with a red dot 🔴

2. **Open Connection Dialog**
   - Click `Connection > Connect...` in the menu bar
   - Fill in your credentials:
   
   ```
   Host:     localhost
   Port:     5432
   Database: postgres
   User:     postgres
   Password: your_password
   ```

3. **Connect**
   - Click the `Connect` button
   - Status should change to green 🟢
   - Databases appear in the left panel

## First Query

1. **Explore Database**
   - Click ▶ next to a database in the tree
   - Click ▶ next to a schema (e.g., "public")
   - Click on any table to view its data

2. **Run Custom Query**
   - Type in the SQL Query editor:
   ```sql
   SELECT * FROM information_schema.tables LIMIT 10;
   ```
   - Click `▶ Execute`
   - Results appear below

## Common Tasks

### View Table Data
- Click on table name in explorer → data loads automatically
- Use Previous/Next for pagination

### Count Rows
```sql
SELECT COUNT(*) FROM your_table;
```

### Filter Data
```sql
SELECT * FROM users WHERE created_at > '2024-01-01' LIMIT 100;
```

### Insert Data
```sql
INSERT INTO users (name, email) VALUES ('John Doe', 'john@example.com');
```

### Update Data
```sql
UPDATE users SET last_login = NOW() WHERE email = 'john@example.com';
```

## Keyboard Tips

- **Tab**: Navigate between fields
- **Escape**: Close dialogs
- **Ctrl+C**: Copy (standard)

## Troubleshooting

### Can't Connect?

**Check PostgreSQL is running:**
```bash
sudo systemctl status postgresql
# or
pg_isready
```

**Test connection manually:**
```bash
psql -h localhost -p 5432 -U postgres -d postgres
```

### Wrong Password?

1. Reset PostgreSQL password:
```bash
sudo -u postgres psql
ALTER USER postgres PASSWORD 'newpassword';
\q
```

2. Try connecting again with new password

### Connection Refused?

Check `postgresql.conf`:
```
listen_addresses = 'localhost'  # or '*' for all
```

Check `pg_hba.conf`:
```
host    all    all    127.0.0.1/32    md5
```

Reload PostgreSQL:
```bash
sudo systemctl reload postgresql
```

## Example Workflow

### Quick Database Check

```sql
-- See all tables
SELECT table_name FROM information_schema.tables 
WHERE table_schema = 'public';

-- Check table size
SELECT COUNT(*) FROM your_table;

-- View recent records
SELECT * FROM your_table 
ORDER BY created_at DESC 
LIMIT 10;
```

### Create Test Table

```sql
-- Create table
CREATE TABLE test_users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    email VARCHAR(255) UNIQUE,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Insert test data
INSERT INTO test_users (name, email) VALUES
    ('Alice', 'alice@test.com'),
    ('Bob', 'bob@test.com'),
    ('Carol', 'carol@test.com');

-- View data
SELECT * FROM test_users;
```

### Clean Up

```sql
DROP TABLE test_users;
```

## Next Steps

- Read [USAGE.md](USAGE.md) for detailed examples
- Check [TODO.md](TODO.md) for upcoming features
- See [README.md](README.md) for full documentation

## Need Help?

- Check PostgreSQL logs: `/var/log/postgresql/`
- Enable debug mode: `RUST_LOG=debug cargo run`
- Review error messages in status bar (bottom of window)

---

Happy querying! 🚀