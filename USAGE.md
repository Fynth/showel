# Showel Usage Guide

## Table of Contents
- [Getting Started](#getting-started)
- [Connection Examples](#connection-examples)
- [Common SQL Queries](#common-sql-queries)
- [Tips and Tricks](#tips-and-tricks)
- [Troubleshooting](#troubleshooting)

## Getting Started

### First Time Setup

1. **Launch Showel**
   ```bash
   cd showel
   cargo run --release
   ```

2. **Connect to PostgreSQL**
   - Click `Connection > Connect...` from the menu
   - Enter your database credentials
   - Click `Connect`

### Quick Start Example

For local PostgreSQL with default settings:
- Host: `localhost`
- Port: `5432`
- Database: `postgres`
- User: `postgres`
- Password: `your_password`

## Connection Examples

### Local Development Database

```
Host: localhost
Port: 5432
Database: myapp_development
User: developer
Password: dev123
```

### Remote Database

```
Host: db.example.com
Port: 5432
Database: production_db
User: readonly_user
Password: secure_password
```

### Docker Container

If running PostgreSQL in Docker:

```
Host: localhost
Port: 5432
Database: postgres
User: postgres
Password: password
```

**Note**: Make sure the port is exposed:
```bash
docker run -p 5432:5432 -e POSTGRES_PASSWORD=password postgres
```

## Common SQL Queries

### Database Exploration

#### List All Tables in Current Schema
```sql
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public'
ORDER BY table_name;
```

#### Get Table Row Count
```sql
SELECT 
    schemaname,
    tablename,
    n_live_tup as row_count
FROM pg_stat_user_tables
ORDER BY n_live_tup DESC;
```

#### Show Table Structure
```sql
SELECT 
    column_name,
    data_type,
    character_maximum_length,
    is_nullable,
    column_default
FROM information_schema.columns
WHERE table_name = 'your_table_name'
ORDER BY ordinal_position;
```

#### List All Schemas
```sql
SELECT schema_name 
FROM information_schema.schemata
WHERE schema_name NOT IN ('pg_catalog', 'information_schema')
ORDER BY schema_name;
```

### Data Queries

#### Select with Limit
```sql
SELECT * FROM users LIMIT 10;
```

#### Select with Condition
```sql
SELECT id, name, email 
FROM users 
WHERE created_at > '2024-01-01'
ORDER BY created_at DESC;
```

#### Join Tables
```sql
SELECT 
    u.id,
    u.name,
    o.order_id,
    o.total
FROM users u
JOIN orders o ON u.id = o.user_id
WHERE o.status = 'completed'
LIMIT 50;
```

#### Aggregate Functions
```sql
SELECT 
    status,
    COUNT(*) as count,
    AVG(total) as average_total,
    SUM(total) as total_sum
FROM orders
GROUP BY status
ORDER BY count DESC;
```

### Data Modification

#### Insert Single Row
```sql
INSERT INTO users (name, email, created_at)
VALUES ('John Doe', 'john@example.com', NOW());
```

#### Insert Multiple Rows
```sql
INSERT INTO products (name, price, category)
VALUES 
    ('Product A', 19.99, 'Electronics'),
    ('Product B', 29.99, 'Electronics'),
    ('Product C', 9.99, 'Books');
```

#### Update Records
```sql
UPDATE users 
SET last_login = NOW()
WHERE email = 'john@example.com';
```

#### Delete Records
```sql
DELETE FROM temp_data 
WHERE created_at < NOW() - INTERVAL '30 days';
```

### Advanced Queries

#### Common Table Expressions (CTE)
```sql
WITH active_users AS (
    SELECT * FROM users 
    WHERE last_login > NOW() - INTERVAL '7 days'
)
SELECT 
    au.name,
    COUNT(o.id) as order_count
FROM active_users au
LEFT JOIN orders o ON au.id = o.user_id
GROUP BY au.name
ORDER BY order_count DESC;
```

#### Window Functions
```sql
SELECT 
    name,
    salary,
    department,
    AVG(salary) OVER (PARTITION BY department) as dept_avg_salary,
    RANK() OVER (PARTITION BY department ORDER BY salary DESC) as dept_rank
FROM employees;
```

#### Subqueries
```sql
SELECT name, email
FROM users
WHERE id IN (
    SELECT DISTINCT user_id 
    FROM orders 
    WHERE total > 100
);
```

### Database Administration

#### Check Database Size
```sql
SELECT 
    pg_database.datname,
    pg_size_pretty(pg_database_size(pg_database.datname)) AS size
FROM pg_database
ORDER BY pg_database_size(pg_database.datname) DESC;
```

#### Check Table Sizes
```sql
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

#### Show Active Connections
```sql
SELECT 
    datname,
    usename,
    application_name,
    client_addr,
    state,
    query_start
FROM pg_stat_activity
WHERE state = 'active'
ORDER BY query_start;
```

#### View Indexes
```sql
SELECT
    tablename,
    indexname,
    indexdef
FROM pg_indexes
WHERE schemaname = 'public'
ORDER BY tablename, indexname;
```

## Tips and Tricks

### Keyboard Navigation

- **Ctrl+Enter**: Execute query (when implemented)
- **Tab**: Navigate between UI elements
- **Escape**: Close dialogs

### Query Editor Tips

1. **Multiple Statements**: Separate statements with semicolons
   ```sql
   SELECT COUNT(*) FROM users;
   SELECT COUNT(*) FROM orders;
   ```

2. **Comments**: Use SQL comments for documentation
   ```sql
   -- This is a single line comment
   SELECT * FROM users; -- inline comment
   
   /*
    * Multi-line comment
    * for complex queries
    */
   SELECT * FROM orders;
   ```

3. **Clear Results**: Click "Clear" to reset the query editor

### Working with Large Tables

When working with tables with millions of rows:

1. **Always Use LIMIT**
   ```sql
   SELECT * FROM huge_table LIMIT 100;
   ```

2. **Use Pagination**
   ```sql
   -- Page 1
   SELECT * FROM huge_table LIMIT 100 OFFSET 0;
   
   -- Page 2
   SELECT * FROM huge_table LIMIT 100 OFFSET 100;
   ```

3. **Filter First**
   ```sql
   SELECT * FROM huge_table 
   WHERE created_at > '2024-01-01'
   LIMIT 100;
   ```

### Best Practices

1. **Test on Development First**: Always test destructive queries on a development database

2. **Use Transactions** (for multiple changes):
   ```sql
   BEGIN;
   UPDATE accounts SET balance = balance - 100 WHERE id = 1;
   UPDATE accounts SET balance = balance + 100 WHERE id = 2;
   COMMIT;
   ```

3. **Backup Before Bulk Changes**:
   ```sql
   -- Create backup table
   CREATE TABLE users_backup AS SELECT * FROM users;
   
   -- Make changes
   UPDATE users SET status = 'inactive' WHERE last_login < '2023-01-01';
   
   -- If something goes wrong, restore
   -- TRUNCATE users;
   -- INSERT INTO users SELECT * FROM users_backup;
   ```

4. **Use EXPLAIN for Slow Queries**:
   ```sql
   EXPLAIN ANALYZE
   SELECT * FROM orders 
   WHERE user_id = 123 
   AND created_at > '2024-01-01';
   ```

## Troubleshooting

### Cannot Connect to Database

**Problem**: "Connection refused" error

**Solutions**:
1. Check if PostgreSQL is running:
   ```bash
   sudo systemctl status postgresql
   # or
   pg_isready
   ```

2. Verify PostgreSQL is listening on the correct port:
   ```bash
   sudo netstat -plnt | grep postgres
   ```

3. Check `postgresql.conf` for `listen_addresses`:
   ```
   listen_addresses = '*'  # or 'localhost' for local only
   ```

4. Check `pg_hba.conf` for connection permissions

### Authentication Failed

**Problem**: "Password authentication failed"

**Solutions**:
1. Verify username and password
2. Check `pg_hba.conf` authentication method:
   ```
   # IPv4 local connections:
   host    all             all             127.0.0.1/32            md5
   ```
3. Reload PostgreSQL after changes:
   ```bash
   sudo systemctl reload postgresql
   ```

### Query Timeout

**Problem**: Query takes too long or hangs

**Solutions**:
1. Add `LIMIT` clause to reduce result set
2. Check for missing indexes:
   ```sql
   SELECT * FROM pg_stat_user_tables WHERE idx_scan = 0;
   ```
3. Simplify complex joins
4. Use `EXPLAIN` to understand query execution

### Connection Drops

**Problem**: Connection lost during operation

**Solutions**:
1. Check network connectivity
2. Verify PostgreSQL `statement_timeout` setting
3. Check server logs for errors
4. Reconnect using `Connection > Connect...`

### Table Not Visible

**Problem**: Cannot see table in explorer

**Solutions**:
1. Verify you're looking in the correct schema
2. Check table ownership:
   ```sql
   SELECT * FROM information_schema.tables 
   WHERE table_name = 'your_table';
   ```
3. Verify user permissions:
   ```sql
   SELECT * FROM information_schema.table_privileges 
   WHERE grantee = 'your_username';
   ```
4. Click `View > Refresh` to reload database structure

### Performance Issues

**Problem**: Application is slow or unresponsive

**Solutions**:
1. Limit result set size (use LIMIT)
2. Close unused connections
3. Restart the application
4. Check system resources (CPU, memory)
5. Use indexes on frequently queried columns

## Example Workflow

### Setting Up a New Database

```sql
-- 1. Create database (connect to 'postgres' first)
CREATE DATABASE myapp;

-- 2. Connect to new database
-- (Use Connection > Connect... and change database to 'myapp')

-- 3. Create schema
CREATE SCHEMA app;

-- 4. Create tables
CREATE TABLE app.users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

-- 5. Insert sample data
INSERT INTO app.users (name, email)
VALUES 
    ('Alice Johnson', 'alice@example.com'),
    ('Bob Smith', 'bob@example.com'),
    ('Carol White', 'carol@example.com');

-- 6. Query data
SELECT * FROM app.users ORDER BY created_at DESC;
```

### Daily Operations

1. **Morning Check** - Verify system health:
   ```sql
   SELECT COUNT(*) FROM important_table;
   SELECT MAX(created_at) FROM logs;
   ```

2. **Data Analysis** - Run reports:
   ```sql
   SELECT DATE(created_at), COUNT(*) 
   FROM orders 
   GROUP BY DATE(created_at)
   ORDER BY DATE(created_at) DESC
   LIMIT 7;
   ```

3. **Maintenance** - Clean up old data:
   ```sql
   DELETE FROM logs WHERE created_at < NOW() - INTERVAL '90 days';
   VACUUM ANALYZE logs;
   ```

---

For more information, see the main [README.md](README.md)