# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- Syntax highlighting in SQL editor
- Auto-completion for SQL keywords and table names
- Query history
- Export results to CSV/JSON
- Dark theme support
- Multiple query tabs
- Connection profiles
- Table structure viewer

## [0.3.0] - 2024-12-09

### Added
- 🎉 **Column Sorting** - Click column headers to sort data
- Sort indicators (▲ ascending / ▼ descending)
- Toggle sort direction by clicking same column again
- Visual feedback with bold headers for sorted columns
- Sorting hint "🔼 Click column header to sort"
- Persistent sort when paginating through data

### Improved
- Clickable column headers in results table
- Visual distinction for sorted columns
- Automatic reload when sort changes
- ORDER BY clause added to SQL queries
- Works seamlessly with pagination

### Technical
- Added `sort_column` and `sort_ascending` to ResultsTable
- Modified `get_table_data()` to accept sort parameters
- Updated `LoadTableData` command with sorting info
- Dynamic ORDER BY clause generation
- Sort state preserved across page navigation

## [0.2.1] - 2024-12-09

### Added
- Type-aware editing - system now detects column data types
- Boolean editor with true/false toggle buttons (✓ true / ✗ false)
- Numeric type validation - warns if invalid number entered
- NULL value support - "Set NULL" button for nullable fields
- Type display in edit dialog shows column data type

### Improved
- Edit dialog now shows column type (e.g., "Type: boolean")
- Boolean fields show as clickable buttons instead of text input
- Numeric fields show validation hints
- Better NULL handling in UPDATE queries
- Visual indicators for boolean values (✓/✗)

### Technical
- Added `get_column_types()` method to retrieve column data types
- Added `GetColumnTypes` command and `ColumnTypes` response
- Type-aware UI rendering in EditDialog
- Proper NULL handling in parameterized queries
- Validation for numeric input

## [0.2.0] - 2024-12-09

### Added
- 🎉 **Cell Editing Feature** - Double-click any cell to edit its value
- Edit dialog with visual feedback and validation
- Automatic primary key detection for UPDATE queries
- Fallback to first column (usually 'id') if no primary key found
- Visual hint "💡 Double-click a cell to edit" in results table
- Cell selection highlighting
- Immediate UI update after editing
- Proper UPDATE query generation with parameterized values

### Improved
- Enhanced table interaction - cells are now clickable
- Better visual feedback for selected cells
- Edit dialog shows original and new values
- Warning when value has changed
- Automatic table reload after successful update

### Technical
- Added `EditDialog` component for cell editing
- Added `update_cell` method in DatabaseConnection
- Added `UpdateCell` command and `CellUpdated` response
- Primary key detection using pg_index and pg_attribute
- Proper SQL injection prevention with parameterized queries

## [0.1.3] - 2024-12-09

### Fixed
- Fixed scrolling behavior - now properly scrolls results table instead of query editor
- Fixed vertical scrolling moving query editor instead of results
- Both horizontal and vertical scrolling now work correctly in results table

### Changed
- Reverted to ScrollArea::both() wrapper for TableBuilder for proper scroll handling
- Added unique id_source to scroll areas to prevent conflicts
- Improved layout structure with explicit vertical container
- Removed lock_focus from query editor to prevent scroll capture

## [0.1.2] - 2024-12-09

### Fixed
- Fixed horizontal scroll issue where SQL editor was scrolling instead of results table
- Results table now has proper horizontal and vertical scrolling

### Improved
- Query editor is now collapsible (click ▶/▼ button)
- Query editor height adjusts dynamically based on content
- Results table uses more screen space efficiently
- Better layout with improved space allocation for results
- Added row count display in results header

### Changed
- Removed nested ScrollArea from results table for better scroll behavior
- TableBuilder now handles scrolling directly with vscroll enabled
- Query editor shows scroll bar only when needed

## [0.1.1] - 2024-12-09

### Fixed
- Fixed runtime panic when starting application (Cannot start a runtime from within a runtime)
- Refactored database operations to use message passing with channels instead of blocking runtime
- Improved async architecture with dedicated worker thread for database operations
- Removed unsafe static variable usage for connection checking

### Changed
- Database operations now run in a separate thread with tokio runtime
- UI thread communicates with database thread via channels
- Non-blocking UI updates with periodic response processing

## [0.1.0] - 2024-12-09

### Added
- Initial release
- PostgreSQL database connection support
- Connection dialog with host, port, database, user, and password fields
- Database explorer with tree view
- Navigate databases, schemas, and tables
- Table data viewer with pagination (100 rows per page)
- SQL query editor with multi-line support
- Execute custom SQL queries (SELECT, INSERT, UPDATE, DELETE)
- Results display in tabular format with resizable columns
- Status bar with connection status and feedback messages
- Error message display with clear button
- Menu system (Connection, View, Help)
- Connect/Disconnect functionality
- Cross-platform support (Linux, macOS, Windows)
- Async database operations with tokio
- Immediate mode GUI with egui
- Comprehensive documentation (README, QUICKSTART, USAGE, etc.)

### Technical Details
- Built with Rust 1.70+
- egui 0.27 for GUI
- tokio-postgres 0.7 for database connectivity
- ~1000 lines of Rust code
- ~17MB release binary size
- ~30-50MB memory usage

### Known Limitations
- Single connection per session
- No query cancellation
- UI blocks during long queries
- No TLS/SSL support
- No credential persistence
- Basic error handling

## Development Notes

### Version 0.1.0 Focus
This initial release focuses on core functionality:
- Reliable PostgreSQL connectivity
- Basic database navigation
- Query execution and results display
- Clean, responsive user interface

### Future Versions
See [TODO.md](TODO.md) for planned features and improvements.

---

**Note**: This is a working prototype suitable for personal use and development workflows. Not recommended for production environments without additional security and stability improvements.

[Unreleased]: https://github.com/yourusername/showel/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/showel/releases/tag/v0.1.0