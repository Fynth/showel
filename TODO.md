# Showel TODO List

## High Priority

### Core Functionality
- [ ] **Query Cancellation** - Ability to stop long-running queries
- [ ] **Connection Pooling** - Support multiple simultaneous connections
- [ ] **Transaction Support** - BEGIN, COMMIT, ROLLBACK with UI controls
- [ ] **Query History** - Save and retrieve previously executed queries
- [ ] **Connection Profiles** - Save and manage database connections

### UI/UX Improvements
- [ ] **Syntax Highlighting** - Color-coded SQL syntax in editor
- [ ] **Auto-completion** - Suggest table names, column names, SQL keywords
- [ ] **Line Numbers** - Show line numbers in query editor
- [ ] **Error Highlighting** - Highlight syntax errors in editor
- [ ] **Result Set Search** - Filter/search within query results
- [ ] **Column Resize** - Remember column widths in results table
- [ ] **Dark/Light Theme Toggle** - User-selectable theme
- [ ] **Font Size Controls** - Adjustable text size in editor and results

### Database Features
- [ ] **Table Structure Viewer** - Show columns, types, constraints, indexes
- [ ] **Foreign Key Navigation** - Click to navigate to referenced tables
- [ ] **View Definitions** - Display and edit view SQL
- [ ] **Stored Procedures** - List and execute functions/procedures
- [ ] **Triggers Viewer** - Display table triggers
- [ ] **Index Management** - View and analyze indexes
- [ ] **User/Role Management** - View database users and permissions

## Medium Priority

### Data Operations
- [ ] **Export Results** - Export to CSV, JSON, Excel formats
- [ ] **Import Data** - Import CSV files into tables
- [ ] **Inline Editing** - Edit cell values directly in results table
- [ ] **Bulk Operations** - Multi-row insert, update, delete
- [ ] **Copy to Clipboard** - Copy results with formatting

### Query Editor
- [ ] **Multiple Query Tabs** - Work with multiple queries simultaneously
- [ ] **Query Formatting** - Auto-format SQL queries
- [ ] **Query Templates** - Pre-built query templates
- [ ] **Query Snippets** - Save and reuse code snippets
- [ ] **SQL Validation** - Validate syntax before execution
- [ ] **Keyboard Shortcuts** - Customizable shortcuts

### Performance
- [ ] **Async UI Updates** - Non-blocking database operations
- [ ] **Streaming Results** - Load large result sets incrementally
- [ ] **Query Caching** - Cache frequently used queries
- [ ] **Lazy Loading** - Load tree items on demand
- [ ] **Virtual Scrolling** - Handle large result sets efficiently

### Database Management
- [ ] **Backup/Restore** - pg_dump/pg_restore integration
- [ ] **Schema Compare** - Compare schemas between databases
- [ ] **Data Migration** - Copy data between databases
- [ ] **Database Creation** - Create new databases from UI
- [ ] **Table Designer** - Visual table creation/modification

## Low Priority

### Advanced Features
- [ ] **Visual Query Builder** - Drag-and-drop query construction
- [ ] **ER Diagram** - Generate entity-relationship diagrams
- [ ] **Query Performance Analysis** - EXPLAIN visualization
- [ ] **Query Optimization Hints** - Suggest query improvements
- [ ] **Real-time Monitoring** - Monitor active queries and connections
- [ ] **Schema Version Control** - Track schema changes
- [ ] **Data Comparison** - Compare data between tables

### Integration
- [ ] **SSH Tunneling** - Connect through SSH tunnel
- [ ] **SSL/TLS Support** - Secure connections
- [ ] **Environment Variables** - Load connection config from env
- [ ] **Configuration Files** - Import/export settings
- [ ] **Command Line Interface** - CLI for automation

### Multi-Database Support
- [ ] **MySQL/MariaDB** - Support MySQL databases
- [ ] **SQLite** - Support embedded SQLite
- [ ] **Microsoft SQL Server** - Support SQL Server
- [ ] **Oracle** - Support Oracle databases
- [ ] **Multi-DB Sessions** - Connect to different DB types simultaneously

### Documentation
- [ ] **In-app Help** - Built-in documentation
- [ ] **Tutorial Mode** - Interactive tutorial for new users
- [ ] **SQL Reference** - Quick reference guide
- [ ] **Keyboard Shortcuts Reference** - Help overlay
- [ ] **Video Tutorials** - Screen recordings

### Testing
- [ ] **Unit Tests** - Test core functionality
- [ ] **Integration Tests** - Test database operations
- [ ] **UI Tests** - Test user interface
- [ ] **Performance Tests** - Benchmark operations
- [ ] **Cross-platform Tests** - Test on Windows, macOS, Linux

### Distribution
- [ ] **Binary Releases** - Pre-built binaries for all platforms
- [ ] **Installer** - Platform-specific installers
- [ ] **Auto-update** - Automatic update checking
- [ ] **Package Managers** - Homebrew, apt, chocolatey
- [ ] **Docker Image** - Containerized version

## Bug Fixes

### Known Issues
- [ ] Fix: Connection status sometimes doesn't update immediately
- [ ] Fix: Error messages can be truncated in status bar
- [ ] Fix: Tree expansion state not preserved after refresh
- [ ] Fix: Large result sets cause UI freeze
- [ ] Fix: Special characters in table names not properly escaped

## Code Quality

### Refactoring
- [ ] Separate async runtime from UI thread
- [ ] Implement proper error types with thiserror
- [ ] Add comprehensive logging
- [ ] Extract database operations to separate service layer
- [ ] Implement state management pattern
- [ ] Add configuration system
- [ ] Create plugin architecture for extensibility

### Documentation
- [ ] Add inline code documentation
- [ ] Document public API
- [ ] Create architecture guide
- [ ] Add contributing guidelines
- [ ] Write code style guide

## Ideas / Brainstorming

- **Collaborative Features** - Share queries with team members
- **Query Scheduler** - Schedule queries to run automatically
- **Data Visualization** - Charts and graphs from query results
- **Custom Themes** - User-defined color schemes
- **Extensions/Plugins** - Support for community extensions
- **Mobile Companion App** - View-only mobile client
- **Web Version** - WASM-based web application
- **AI Assistant** - Natural language to SQL conversion
- **Data Masking** - Hide sensitive data in results
- **Audit Log** - Track all database operations
- **Bookmarks** - Bookmark favorite tables/queries
- **Notes/Comments** - Add notes to connections/tables
- **Workspace Save/Restore** - Save entire work session
- **Multi-language Support** - Internationalization

## Completed ✓

- [x] Basic PostgreSQL connection
- [x] Database tree navigation
- [x] Table browsing with pagination
- [x] SQL query execution
- [x] Results display in table format
- [x] Connection dialog
- [x] Basic error handling
- [x] Status bar with feedback
- [x] Menu system

---

**Note**: This is a living document. Priorities may change based on user feedback and project goals.

Last updated: 2024