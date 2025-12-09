# Column Sorting Feature v0.3.0

## New Feature: Sort by Clicking Headers! 🔼

Теперь можно сортировать данные простым кликом по заголовку колонки!

## How to Use

### Quick Guide

1. **View table data** - Open any table or execute query
2. **Click column header** - Sort ascending (▲)
3. **Click again** - Toggle to descending (▼)
4. **Click another column** - Sort by different column

### Visual Indicators

```
Results Table:
┌─────────────────────────────────────────────────────┐
│ Results: 100 rows | 💡 Double-click to edit         │
│                   | 🔼 Click column to sort         │
├─────────────────────────────────────────────────────┤
│ [id ▲] [name] [email] [created_at]                  │
├─────────────────────────────────────────────────────┤
│ 1     Alice   alice@...   2024-01-15                │
│ 2     Bob     bob@...     2024-01-16                │
│ 3     Carol   carol@...   2024-01-17                │
│ ...                                                  │
└─────────────────────────────────────────────────────┘
```

**Indicators:**
- `▲` - Sorted ascending (A→Z, 1→9, old→new)
- `▼` - Sorted descending (Z→A, 9→1, new→old)
- **Bold** - Currently sorted column
- Normal - Unsorted columns

## Features

### 1. Click to Sort

**Single click on header:**
- First click: Sort ascending ▲
- Second click: Sort descending ▼
- Click different header: Sort by that column

### 2. Visual Feedback

**Sorted column:**
- Bold text
- Arrow indicator (▲/▼)
- Stands out from other headers

**Other columns:**
- Normal text
- Clickable
- No arrow

### 3. Persistent Sorting

**Across pages:**
- Sort is remembered
- Works with pagination
- Same order on all pages

### 4. Auto-reload

**On sort change:**
- Table reloads automatically
- Data sorted in database
- No manual refresh needed

## Examples

### Sort by Name (Ascending)

```
Before:
id | name    | email
---+---------+----------------
3  | Carol   | carol@...
1  | Alice   | alice@...
2  | Bob     | bob@...

After clicking "name" header:
id | name ▲  | email
---+---------+----------------
1  | Alice   | alice@...
2  | Bob     | bob@...
3  | Carol   | carol@...
```

### Sort by ID (Descending)

```
After clicking "id ▲" header again:
id ▼| name   | email
----+--------+----------------
3   | Carol  | carol@...
2   | Bob    | bob@...
1   | Alice  | alice@...
```

### Sort by Date

```
created_at ▼ (newest first)
------------------------
2024-01-20
2024-01-18
2024-01-15
2024-01-10
```

## How It Works

### 1. Click Detection

When header is clicked:

```rust
// In ResultsTable::show()
if ui.button(format!("{} {}", column, arrow)).clicked() {
    sort_by_column = Some(col_idx);
}
```

### 2. Sort State Update

```rust
if self.sort_column == Some(col_idx) {
    // Same column - toggle direction
    self.sort_ascending = !self.sort_ascending;
} else {
    // New column - start ascending
    self.sort_column = Some(col_idx);
    self.sort_ascending = true;
}
```

### 3. SQL Query Generation

```rust
// In get_table_data()
let mut query = format!("SELECT * FROM {}.{}", schema, table);

if let Some(col) = sort_column {
    let direction = if sort_ascending { "ASC" } else { "DESC" };
    query.push_str(&format!(" ORDER BY {} {}", col, direction));
}

query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
```

**Generated SQL:**
```sql
-- Ascending
SELECT * FROM public.users ORDER BY name ASC LIMIT 100 OFFSET 0

-- Descending
SELECT * FROM public.users ORDER BY id DESC LIMIT 100 OFFSET 0
```

### 4. Auto-reload Trigger

```rust
// In app.rs update()
let prev_sort = results_table.get_sort_info();

// ... show table ...

let new_sort = results_table.get_sort_info();
if prev_sort != new_sort {
    // Sort changed - reload data
    self.load_table_data(schema, table);
}
```

## Technical Implementation

### Data Structures

```rust
pub struct ResultsTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub sort_column: Option<usize>,     // Which column
    pub sort_ascending: bool,           // Direction
    // ...
}
```

### Methods

```rust
impl ResultsTable {
    // Get current sort info
    pub fn get_sort_info(&self) -> Option<(String, bool)> {
        self.sort_column.map(|idx| {
            (self.columns[idx].clone(), self.sort_ascending)
        })
    }
}
```

### Database Layer

```rust
pub async fn get_table_data(
    &self,
    schema: &str,
    table: &str,
    limit: i64,
    offset: i64,
    sort_column: Option<&str>,    // New parameter
    sort_ascending: bool,          // New parameter
) -> Result<QueryResult>
```

### Command/Response

```rust
enum DbCommand {
    LoadTableData(
        String,        // schema
        String,        // table
        i64,           // limit
        i64,           // offset
        Option<String>,// sort_column
        bool,          // sort_ascending
    ),
}
```

## Sort Types

### Text Columns
- Alphabetical order
- A→Z (ascending)
- Z→A (descending)
- Case-sensitive by default

### Numeric Columns
- Numerical order
- 1→999 (ascending)
- 999→1 (descending)
- NULL values last

### Date/Time Columns
- Chronological order
- Oldest→Newest (ascending)
- Newest→Oldest (descending)

### Boolean Columns
- false→true (ascending)
- true→false (descending)

## Benefits

### User Experience
- ✅ Quick data analysis
- ✅ Find min/max values instantly
- ✅ No need to write ORDER BY
- ✅ Visual feedback

### Performance
- ✅ Database-side sorting (efficient)
- ✅ Uses indexes when available
- ✅ Works with pagination
- ✅ Fast for large tables

### Usability
- ✅ Intuitive (like Excel/spreadsheets)
- ✅ One click to sort
- ✅ Clear indicators
- ✅ Works immediately

## Limitations

### Current Version

1. **Single column sort only** - No multi-column sorting yet
2. **Case-sensitive text** - Uppercase comes before lowercase
3. **No custom collations** - Uses database default
4. **SQL query results** - Sorting works best with table views

### Workarounds

**Multi-column sort:**
- Use SQL query: `SELECT ... ORDER BY col1, col2`

**Case-insensitive:**
- Use SQL query: `SELECT ... ORDER BY LOWER(column)`

**Custom order:**
- Use SQL query with CASE statements

## Examples in Action

### Find Highest Price

```
1. Open "products" table
2. Click "price" header
3. Click again to sort descending ▼
4. See most expensive items first
```

### Find Newest Records

```
1. Open any table with "created_at"
2. Click "created_at" header
3. Click again to sort descending ▼
4. See newest records first
```

### Alphabetical List

```
1. Open "users" table
2. Click "name" header
3. See names in A→Z order ▲
```

### Find Empty Values

```
1. Sort by column
2. NULL values typically at end
3. Scroll to see empty/NULL entries
```

## Testing

```bash
# Build
cargo build --release

# Test
1. Connect to PostgreSQL
2. Open any table
3. Click on column header
4. Verify:
   - Arrow appears (▲)
   - Data is sorted
   - Header is bold
5. Click same header again
6. Verify:
   - Arrow flips (▼)
   - Data reversed
7. Click different header
8. Verify:
   - New column sorted
   - Previous loses bold
9. Navigate pages
10. Verify sort persists
```

## Keyboard Support (Future)

Planned for future versions:

- [ ] Shift+Click for secondary sort
- [ ] Ctrl+Click for multi-column sort
- [ ] Arrow keys to navigate headers
- [ ] Enter to toggle sort on focused header

## Performance Tips

### For Large Tables

1. **Use indexes** - Create indexes on frequently sorted columns:
   ```sql
   CREATE INDEX idx_users_created_at ON users(created_at);
   ```

2. **Limit results** - Keep page size reasonable (100-1000 rows)

3. **Avoid sorting text** - Text sorting can be slower than numbers

### For Best Performance

- Sort on indexed columns
- Use numeric/date columns when possible
- Keep page size moderate
- Consider materialized views for complex sorts

## Code Changes

### Files Modified

```
src/ui.rs       505 → 535 строк  (+30)
src/db.rs       378 → 390 строк  (+12)
src/app.rs      528 → 545 строк  (+17)
Cargo.toml      v0.2.1 → v0.3.0
CHANGELOG.md    Updated
```

### New Features

- Clickable column headers
- Sort state tracking
- ORDER BY query generation
- Visual indicators (▲/▼)
- Bold sorted columns

---

**Version**: 0.3.0  
**Date**: 2024-12-09  
**Feature**: Column Sorting  
**Status**: ✅ Production Ready  
**Type**: Major Feature Release
