# Showel - Project Overview

## What is Showel?

Showel is a lightweight, native desktop application for managing PostgreSQL databases. Built with Rust and egui, it provides a fast, cross-platform alternative to heavy database management tools like DBeaver, with a focus on simplicity and performance.

## Why Showel?

- **Fast**: Native Rust application with minimal overhead
- **Simple**: Clean, intuitive interface focused on essential features
- **Lightweight**: Low memory footprint, quick startup
- **Cross-platform**: Works on Linux, macOS, and Windows
- **PostgreSQL-focused**: Optimized for PostgreSQL workflows

## Project Structure

```
showel/
├── src/
│   ├── main.rs          # Application entry point, eframe setup
│   ├── app.rs           # Main application logic and state management
│   ├── db.rs            # Database connection and PostgreSQL operations
│   └── ui.rs            # UI components (dialogs, tables, tree view)
├── Cargo.toml           # Dependencies and project configuration
├── README.md            # Main documentation
├── QUICKSTART.md        # 5-minute getting started guide
├── USAGE.md             # Detailed usage examples and SQL queries
├── TODO.md              # Feature roadmap and improvements
└── OVERVIEW.md          # This file
```

## Architecture

### Technology Stack

| Component | Technology | Purpose |
|-----------|------------|---------|
| GUI Framework | egui 0.27 | Immediate mode GUI |
| Application Framework | eframe 0.27 | Window management and main loop |
| Database Client | tokio-postgres 0.7 | PostgreSQL connectivity |
| Async Runtime | tokio 1.0 | Asynchronous operations |
| Serialization | serde 1.0 | Data serialization |
| Error Handling | anyhow, thiserror | Error management |
| Logging | tracing | Debug and info logging |

### Core Components

#### 1. Database Layer (`db.rs`)

**DatabaseConnection**
- Manages PostgreSQL connections using tokio-postgres
- Provides async methods for database operations
- Handles connection state and configuration

**Key Features:**
- Connection management (connect, disconnect, status)
- Database introspection (databases, schemas, tables)
- Query execution (SELECT, INSERT, UPDATE, DELETE)
- Table data retrieval with pagination
- Row counting for large tables

**Data Structures:**
- `ConnectionConfig`: Database connection parameters
- `QueryResult`: Query execution results with columns and rows
- `ColumnInfo`: Table column metadata

#### 2. Application Layer (`app.rs`)

**ShowelApp**
- Main application state and logic
- Implements eframe::App trait for the main update loop
- Manages UI state and user interactions
- Coordinates between UI and database layers

**State Management:**
- Connection status and configuration
- Database tree expansion state
- Query editor content
- Results table data
- Pagination state for table viewing
- Status messages and errors

**Key Methods:**
- `connect()`: Establish database connection
- `disconnect()`: Close connection and reset state
- `load_databases()`: Fetch list of databases
- `load_schemas()`: Fetch schemas in current database
- `load_tables()`: Fetch tables in schema
- `execute_query()`: Run SQL query and display results
- `load_table_data()`: Load paginated table data

#### 3. UI Layer (`ui.rs`)

**ConnectionDialog**
- Modal dialog for database connection parameters
- Form validation and connection setup

**DatabaseTree**
- Hierarchical tree view of database structure
- Expandable nodes for databases, schemas, tables
- Click handlers for navigation
- Returns `TreeAction` enum for user interactions

**QueryEditor**
- Multi-line SQL editor
- Execute and clear buttons
- Monospace font for code editing

**ResultsTable**
- Tabular display of query results
- Uses egui_extras::TableBuilder for rendering
- Resizable columns, striped rows
- Scroll support for large result sets

### Data Flow

```
User Input → UI Components → App State → Database Layer → PostgreSQL
                ↓                                           ↓
         UI Update ← App State Update ← Query Results ← Database
```

1. **User Action**: Click button, type query, select table
2. **UI Component**: Captures input, returns action/data
3. **App State**: Processes action, calls appropriate method
4. **Database Layer**: Executes async operation
5. **Results**: Data flows back through layers to UI
6. **Render**: egui immediate mode renders updated state

### Asynchronous Operations

- **Architecture**: Database operations run in a dedicated worker thread
- **Runtime**: Separate tokio runtime in worker thread, isolated from UI
- **Communication**: Channels (mpsc) for command/response between UI and worker
- **Execution**: Non-blocking - UI sends commands and processes responses when available
- **Benefits**: UI remains responsive during long database operations

**Implementation Details**:
- `DbCommand` enum: Commands sent from UI to worker (Connect, Query, etc.)
- `DbResponse` enum: Responses sent from worker to UI (Results, Errors, etc.)
- Worker thread runs continuous loop processing commands
- UI thread processes responses via `try_recv()` in update loop

### Connection Management

- Single active connection per session
- Connection state managed in worker thread
- Connection status checked periodically (every 2 seconds)
- Automatic reconnection not implemented (manual reconnect required)
- No connection pooling (single-user application)
- Thread-safe communication via channels ensures safe concurrent access

## Key Features Implemented

### ✅ Completed

- [x] PostgreSQL connection dialog
- [x] Connect/disconnect functionality
- [x] Database explorer tree view
- [x] Schema browsing
- [x] Table listing
- [x] View table data with pagination
- [x] Execute custom SQL queries
- [x] Results display in table format
- [x] Status bar with connection info
- [x] Error message display
- [x] Query result formatting (multiple data types)
- [x] Menu system
- [x] Responsive layout (panels, scrolling)

### 🚧 In Progress / Planned

See [TODO.md](TODO.md) for complete roadmap

**High Priority:**
- Query cancellation
- Connection pooling
- Query history
- Syntax highlighting
- Auto-completion
- Export results (CSV, JSON)

**Medium Priority:**
- Table structure viewer
- Multiple query tabs
- Dark/light theme toggle
- Inline data editing
- Transaction controls

**Low Priority:**
- Visual query builder
- ER diagrams
- Multi-database support (MySQL, SQLite)
- SSH tunneling
- SSL/TLS connections

## Performance Considerations

### Current Performance

- **Startup Time**: < 2 seconds
- **Memory Usage**: ~30-50MB base, scales with result set size
- **Query Execution**: Limited by PostgreSQL and network latency
- **UI Responsiveness**: 60 FPS - remains responsive during queries thanks to async architecture

### Known Limitations

- **Large Result Sets**: Loading 10,000+ rows can cause lag
- **Connection Handling**: No reconnection on connection loss
- **Memory**: Result sets held entirely in memory
- **Error Recovery**: Limited handling of connection drops

### Optimization Opportunities

1. ~~**Async UI**: Separate database operations from UI thread~~ ✅ **Implemented in v0.1.1**
2. **Streaming Results**: Load results incrementally
3. **Virtual Scrolling**: Render only visible rows
4. **Query Cancellation**: Implement cancellable futures
5. **Connection Pooling**: Reuse connections efficiently

## Development Guidelines

### Code Style

- **Formatting**: Use `cargo fmt`
- **Linting**: Use `cargo clippy`
- **Naming**: Follow Rust conventions (snake_case, CamelCase)
- **Error Handling**: Use `Result<T, Error>` with proper context

### Adding New Features

1. **Database Operations**: Add methods to `DatabaseConnection` in `db.rs`
2. **UI Components**: Create new widgets in `ui.rs`
3. **App Logic**: Add state and handlers to `ShowelApp` in `app.rs`
4. **Integration**: Wire components together in `update()` method

### Testing Strategy

**Current State**: No automated tests (manual testing only)

**Recommended Approach**:
- Unit tests for database query building
- Integration tests with test PostgreSQL instance
- UI tests using egui test harness
- Performance benchmarks for large datasets

## Building and Running

### Development Build

```bash
cargo run
```

### Release Build

```bash
cargo build --release
./target/release/showel
```

### With Logging

```bash
RUST_LOG=showel=debug cargo run
```

### Platform-Specific

**Linux**: Requires X11/Wayland and libfontconfig
**macOS**: No additional requirements
**Windows**: No additional requirements

## Dependencies

### Direct Dependencies

- `eframe`: GUI application framework
- `egui`: Immediate mode GUI library
- `egui_extras`: Additional egui widgets (tables)
- `tokio`: Async runtime
- `tokio-postgres`: PostgreSQL driver
- `serde`: Serialization framework
- `serde_json`: JSON support
- `anyhow`: Error handling
- `thiserror`: Custom error types
- `chrono`: Date/time handling
- `tracing`: Logging
- `tracing-subscriber`: Log formatting

### Why These Choices?

- **egui**: Fast, native-feeling UI with minimal boilerplate
- **tokio**: De-facto standard for async Rust
- **tokio-postgres**: Pure Rust, well-maintained, feature-complete
- **anyhow/thiserror**: Best-practice error handling in Rust

## Security Considerations

### Current State

- **Credentials**: Stored in memory only (not persisted)
- **Connections**: No TLS/SSL support (uses NoTls)
- **SQL Injection**: No parameterization for table/schema names
- **Audit**: No logging of executed queries

### Recommendations for Production

1. **Encrypt saved credentials** (when implemented)
2. **Add TLS/SSL support** for secure connections
3. **Implement query auditing** for compliance
4. **Sanitize user input** for table/schema names
5. **Add connection timeout** and retry logic
6. **Implement user permissions** checking

## Contributing

See [TODO.md](TODO.md) for areas needing work.

**Good First Issues**:
- Add keyboard shortcuts
- Implement query history
- Add export to CSV
- Improve error messages
- Add unit tests

**Larger Projects**:
- Syntax highlighting
- Auto-completion
- Connection pooling
- Multi-database support

## License

[Specify your license here]

## Resources

- **egui Documentation**: https://docs.rs/egui/
- **tokio-postgres Docs**: https://docs.rs/tokio-postgres/
- **PostgreSQL Docs**: https://www.postgresql.org/docs/

---

**Project Status**: Working prototype, suitable for personal use. Not recommended for production without additional security and stability improvements.

**Last Updated**: December 2024