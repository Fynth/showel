```
   _____ __                     __
  / ___// /_  ____ _      _____/ /
  \__ \/ __ \/ __ \ | /| / / __  / 
 ___/ / / / / /_/ / |/ |/ / /_/ /  
/____/_/ /_/\____/|__/|__/\__,_/   
                                    
PostgreSQL Database Manager
```

# Showel - PostgreSQL Database Manager

A lightweight, desktop GUI application for managing PostgreSQL databases, built with Rust and egui. Think of it as a simplified, native alternative to DBeaver focused on PostgreSQL.

## Features

- 🔌 **Database Connections**: Connect to PostgreSQL databases with a simple connection dialog
- 🗂️ **Database Explorer**: Browse databases, schemas, and tables in a tree view
- 📊 **Table Viewer**: View table data with pagination support
- 🔍 **SQL Query Editor**: Execute custom SQL queries with a built-in editor
- 📈 **Results Display**: View query results in a clean, tabular format
- ⚡ **Fast & Lightweight**: Built with Rust for performance and low resource usage
- 🖥️ **Native UI**: Cross-platform desktop application using egui

## Prerequisites

- Rust 1.70 or higher
- PostgreSQL server (local or remote)

## Installation

### From Source

```bash
git clone <your-repo-url>
cd showel
cargo build --release
```

The binary will be available at `target/release/showel`

## Usage

### Starting the Application

```bash
cargo run --release
```

Or run the compiled binary directly:

```bash
./target/release/showel
```

### Connecting to a Database

1. Launch Showel
2. Click `Connection > Connect...` in the menu bar
3. Fill in your PostgreSQL connection details:
   - **Host**: Database server address (e.g., `localhost`)
   - **Port**: PostgreSQL port (default: `5432`)
   - **Database**: Database name (e.g., `postgres`)
   - **User**: PostgreSQL username
   - **Password**: User password
4. Click `Connect`

### Exploring Databases

Once connected:

- The **Database Explorer** (left panel) shows available databases
- Click the `▶` icon to expand and view schemas
- Expand schemas to see their tables
- Click on any table to view its data

### Viewing Table Data

- Click on a table in the Database Explorer
- Data is displayed with pagination (100 rows per page by default)
- Use `◀ Previous` and `Next ▶` buttons to navigate pages
- Total row count is displayed in the status bar

### Executing SQL Queries

1. Use the **SQL Query** editor in the main panel
2. Type or paste your SQL query
3. Click `▶ Execute` to run the query
4. Results appear in the **Results** table below
5. Use `Clear` to empty the editor

Example queries:
```sql
-- List all tables
SELECT * FROM information_schema.tables LIMIT 10;

-- Custom query
SELECT * FROM your_table WHERE condition = 'value';

-- Insert data
INSERT INTO your_table (column1, column2) VALUES ('value1', 'value2');

-- Update records
UPDATE your_table SET column1 = 'new_value' WHERE id = 1;
```

### Disconnecting

Click `Connection > Disconnect` to close the current database connection.

## Architecture

### Technology Stack

- **egui**: Immediate mode GUI framework
- **eframe**: Application framework for egui
- **tokio**: Async runtime for database operations
- **tokio-postgres**: PostgreSQL client library
- **serde**: Serialization/deserialization

### Project Structure

```
showel/
├── src/
│   ├── main.rs          # Application entry point
│   ├── app.rs           # Main application logic
│   ├── db.rs            # Database connection and operations
│   └── ui.rs            # UI components (dialogs, tables, tree view)
├── Cargo.toml           # Dependencies and project configuration
└── README.md            # This file
```

### Key Components

- **DatabaseConnection**: Manages PostgreSQL connections and executes queries
- **ShowelApp**: Main application state and update logic
- **ConnectionDialog**: UI for database connection parameters
- **DatabaseTree**: Tree view for browsing database structure
- **QueryEditor**: SQL query input interface
- **ResultsTable**: Display query results in table format

## Building for Production

```bash
cargo build --release --locked
```

### Platform-Specific Notes

#### Linux
```bash
# Install required dependencies (Ubuntu/Debian)
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
                     libxkbcommon-dev libssl-dev libfontconfig1-dev
```

#### macOS
No additional dependencies required. Builds work out of the box.

#### Windows
No additional dependencies required. Builds work out of the box.

## Development

### Running in Debug Mode

```bash
cargo run
```

### Enabling Logging

The application uses `tracing` for logging. Set the log level:

```bash
RUST_LOG=showel=debug cargo run
```

Log levels: `trace`, `debug`, `info`, `warn`, `error`

### Code Structure

- **Async Operations**: Database operations run asynchronously using Tokio
- **Blocking UI**: UI updates use `runtime.block_on()` for simplicity
- **Connection Pooling**: Single connection per session (can be extended)

## Features Roadmap

- [ ] Multiple concurrent database connections
- [ ] Syntax highlighting in SQL editor
- [ ] Export query results (CSV, JSON)
- [ ] Table structure viewer (columns, indexes, constraints)
- [ ] Query history
- [ ] Save connection profiles
- [ ] Dark/Light theme toggle
- [ ] Auto-completion in SQL editor
- [ ] Visual query builder
- [ ] Database schema visualization
- [ ] Backup/Restore functionality

## Known Limitations

- Single connection per session
- No query cancellation support
- Limited error recovery
- Basic SQL editor (no syntax highlighting yet)
- Table pagination is sequential only

## Troubleshooting

### Connection Refused
- Ensure PostgreSQL server is running
- Check if the server allows connections from your IP
- Verify `pg_hba.conf` settings

### Authentication Failed
- Verify username and password
- Check user permissions in PostgreSQL

### SSL Errors
Currently, Showel uses `NoTls` connection mode. For SSL support, this will need to be extended.

## Contributing

Contributions are welcome! Areas for improvement:
- UI/UX enhancements
- Performance optimizations
- Additional PostgreSQL features
- Better error handling
- Tests

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Credits

Built with:
- [egui](https://github.com/emilk/egui) - Immediate mode GUI library
- [tokio-postgres](https://github.com/sfackler/rust-postgres) - PostgreSQL client

---

**Note**: This is a learning project and should not be used in production environments without proper security review and testing.