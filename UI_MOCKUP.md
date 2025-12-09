# Showel UI Mockup

## Application Window Layout

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ Connection    View    Help                                    🟢 Connected      │
├─────────────────────────────────────────────────────────────────────────────────┤
│                 │                                                                │
│  DATABASE       │  SQL Query                                                     │
│  EXPLORER       │  ┌────────────────────────────────────────────────────────┐  │
│                 │  │ SELECT * FROM users                                     │  │
│  ▼ 📊 postgres  │  │ WHERE created_at > '2024-01-01'                        │  │
│    ▼ 📁 public  │  │ ORDER BY created_at DESC                               │  │
│      📋 users   │  │ LIMIT 100;                                             │  │
│      📋 orders  │  │                                                         │  │
│      📋 products│  │                                                         │  │
│                 │  └────────────────────────────────────────────────────────┘  │
│  ▶ 📊 myapp_db  │  [▶ Execute]  [Clear]                                         │
│                 │                                                                │
│  ▶ 📊 testdb    │  ─────────────────────────────────────────────────────────── │
│                 │                                                                │
│                 │  Results                                              Page 1/5 │
│                 │  ┌────┬──────────┬──────────────────┬────────────────────┐   │
│                 │  │ id │ name     │ email            │ created_at         │   │
│                 │  ├────┼──────────┼──────────────────┼────────────────────┤   │
│                 │  │ 1  │ Alice    │ alice@example.com│ 2024-01-15 10:30:00│   │
│                 │  │ 2  │ Bob      │ bob@example.com  │ 2024-01-16 14:20:00│   │
│                 │  │ 3  │ Carol    │ carol@example.com│ 2024-01-17 09:15:00│   │
│                 │  │ 4  │ Dave     │ dave@example.com │ 2024-01-18 11:45:00│   │
│                 │  └────┴──────────┴──────────────────┴────────────────────┘   │
│                 │  [◀ Previous]  [Next ▶]                    Showing 100 rows   │
│                 │                                                                │
├─────────────────────────────────────────────────────────────────────────────────┤
│ Status: Query executed successfully (4 rows)                                    │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Connection Dialog

```
┌───────────────────────────────────┐
│  Connect to Database              │
├───────────────────────────────────┤
│                                   │
│  Host:     [localhost________]    │
│                                   │
│  Port:     [5432____________]     │
│                                   │
│  Database: [postgres_________]    │
│                                   │
│  User:     [postgres_________]    │
│                                   │
│  Password: [••••••••••••_____]    │
│                                   │
│       [Connect]    [Cancel]       │
│                                   │
└───────────────────────────────────┘
```

## Table View (When Clicking on Table)

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                 │                                                                │
│  ▼ 📊 postgres  │  Table: public.users                    Total rows: 1,247     │
│    ▼ 📁 public  │                                                                │
│      📋 users ← │  [◀ Previous]  Page 1 of 13  [Next ▶]    Showing 100 rows    │
│      📋 orders  │                                                                │
│                 │  ┌────┬──────────┬──────────────────┬────────────────────┐   │
│                 │  │ id │ name     │ email            │ created_at         │   │
│                 │  ├────┼──────────┼──────────────────┼────────────────────┤   │
│                 │  │ 1  │ Alice    │ alice@example.com│ 2024-01-15 10:30:00│   │
│                 │  │ 2  │ Bob      │ bob@example.com  │ 2024-01-16 14:20:00│   │
│                 │  │ 3  │ Carol    │ carol@example.com│ 2024-01-17 09:15:00│   │
│                 │  │ ...│ ...      │ ...              │ ...                │   │
│                 │  └────┴──────────┴──────────────────┴────────────────────┘   │
│                 │                                                                │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Error State

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ Connection    View    Help                                    🔴 Not Connected  │
├─────────────────────────────────────────────────────────────────────────────────┤
│                 │                                                                │
│  DATABASE       │  SQL Query                                                     │
│  EXPLORER       │  ┌────────────────────────────────────────────────────────┐  │
│                 │  │ SELECT * FROM non_existent_table;                      │  │
│  Connect to see │  │                                                         │  │
│  databases      │  │                                                         │  │
│                 │  └────────────────────────────────────────────────────────┘  │
│                 │  [▶ Execute]  [Clear]                                         │
│                 │                                                                │
│                 │  ─────────────────────────────────────────────────────────── │
│                 │                                                                │
│                 │  Results                                                       │
│                 │  No results to display. Execute a query to see results.       │
│                 │                                                                │
├─────────────────────────────────────────────────────────────────────────────────┤
│ Query failed ❌ ERROR: relation "non_existent_table" does not exist [Clear]     │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## UI Components Legend

### Icons
- 📊 Database
- 📁 Schema
- 📋 Table
- 🟢 Connected (green)
- 🔴 Disconnected (red)
- ▶ Collapsed/Execute
- ▼ Expanded
- ◀ Previous
- ▶ Next
- ❌ Error
- ✅ Success

### Panels
1. **Top Menu Bar**: Connection, View, Help menus + status indicator
2. **Left Sidebar**: Database explorer tree (resizable)
3. **Main Panel**: Query editor + Results table
4. **Status Bar**: Messages, errors, row counts

### Interactive Elements

#### Database Tree
- Click ▶/▼ to expand/collapse
- Click database name to load schemas
- Click schema name to load tables
- Click table name to view data

#### Query Editor
- Multi-line text input
- Monospace font
- Execute button runs query
- Clear button empties editor

#### Results Table
- Column headers
- Sortable columns (future feature)
- Resizable columns
- Scroll bars for large results
- Pagination controls

#### Status Bar
- Left: Status messages
- Right: Error messages with clear button
- Updates on every action

## Color Scheme (Default)

### Light Theme
- Background: #FFFFFF
- Text: #000000
- Border: #CCCCCC
- Accent: #0066CC
- Success: #00AA00
- Error: #CC0000

### Dark Theme (Future)
- Background: #1E1E1E
- Text: #D4D4D4
- Border: #3E3E3E
- Accent: #4A9EFF
- Success: #00DD00
- Error: #FF4444

## Responsive Behavior

### Window Resize
- Minimum size: 800x600
- Left panel: Min 200px, Max 400px
- Query editor: Min height 100px
- Results table: Takes remaining space

### Large Result Sets
- Pagination controls always visible
- Scroll bars appear when needed
- Column widths adjust to content

### Long Text
- Table cells: Truncate with ellipsis
- Error messages: Wrap to multiple lines
- SQL editor: Scroll horizontally/vertically

## Keyboard Navigation (Future)

- `Ctrl+Enter`: Execute query
- `Ctrl+K`: Clear editor
- `Ctrl+F`: Find in results
- `Ctrl+,`: Open settings
- `F5`: Refresh tree
- `Escape`: Close dialogs

## User Flow Examples

### First Time User
1. Launch app → See "Not Connected" state
2. Click "Connection > Connect..."
3. Fill in connection details
4. Click "Connect"
5. See databases populate in tree
6. Expand database → schema → table
7. Click table → see data
8. Type query in editor
9. Click "Execute" → see results

### Regular User
1. Launch app → Auto-connect (future)
2. Navigate to favorite table
3. Execute saved query (future)
4. Export results (future)
5. Switch to different database
6. Repeat workflow

### Power User
1. Launch with CLI args (future)
2. Open multiple tabs (future)
3. Run complex queries
4. Use keyboard shortcuts (future)
5. Export and analyze results

## Accessibility

- Clear visual hierarchy
- High contrast text
- Keyboard navigation support (future)
- Screen reader support (future)
- Resizable text (future)

## Performance Indicators

- Loading spinner during queries (future)
- Progress bar for large operations (future)
- Query execution time display (future)
- Row count in status bar
- Page number indicator

---

This mockup represents the current implementation and planned features. 
See TODO.md for implementation roadmap.