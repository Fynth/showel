# Cell Editing Feature v0.2.0

## New Feature: Edit Table Cells! 🎉

Теперь Showel поддерживает редактирование данных в таблицах прямо из интерфейса!

## How to Use

### Quick Guide

1. **View table data**: Click on table in database explorer
2. **Select cell**: Double-click any cell you want to edit
3. **Edit value**: Enter new value in the dialog
4. **Save**: Click "Save" to update the database

### Step by Step

```
1. Connect to database
   ├─> Expand schema (e.g., "public")
   └─> Click on table

2. Double-click a cell in results table
   ├─> Edit dialog opens
   ├─> Shows current value
   └─> Enter new value

3. Click "Save"
   ├─> UPDATE query executes
   ├─> Table reloads automatically
   └─> See updated value
```

## Features

### Edit Dialog

```
┌─────────────────────────────────────┐
│  Edit: email                        │
├─────────────────────────────────────┤
│  Column: email                      │
│                                     │
│  Original: alice@example.com        │
│  ─────────────────────────────────  │
│  New value:                         │
│  [alice@newdomain.com_________]     │
│                                     │
│  ⚠ Value will be updated            │
│                                     │
│  [Save]  [Cancel]                   │
└─────────────────────────────────────┘
```

**Features:**
- Shows column name
- Displays original value
- Text input for new value
- Warning when value changed
- Save/Cancel buttons

### Cell Selection

- **Single click**: Select cell (highlights)
- **Double click**: Open edit dialog
- Selected cell shown with highlight

### Visual Hints

- 💡 "Double-click a cell to edit" in results header
- ⚠ Warning when value differs from original
- Immediate UI update after save
- Status message confirms success

## Technical Details

### Primary Key Detection

The system automatically detects the primary key for UPDATE operations:

```sql
-- Query to find primary key
SELECT a.attname FROM pg_index i
JOIN pg_attribute a ON a.attrelid = i.indrelid 
    AND a.attnum = ANY(i.indkey)
WHERE i.indrelid = 'schema.table'::regclass 
    AND i.indisprimary
```

**Fallback:** If no primary key found, uses first column (usually 'id')

### UPDATE Query

```sql
UPDATE schema.table 
SET column_name = $1 
WHERE primary_key = $2
```

**Safe:** Uses parameterized queries to prevent SQL injection

### Architecture

```
User double-clicks cell
    ↓
EditDialog opens
    ↓
User enters new value
    ↓
Click "Save"
    ↓
DbCommand::UpdateCell sent
    ↓
Worker thread executes UPDATE
    ↓
DbResponse::CellUpdated received
    ↓
Table reloads automatically
    ↓
UI shows updated value
```

## Code Components

### 1. EditDialog (ui.rs)

```rust
pub struct EditDialog {
    pub open: bool,
    pub value: String,
    pub original_value: String,
    pub column_name: String,
    pub row_index: usize,
    pub col_index: usize,
}

// Opens dialog with cell info
edit_dialog.open(value, column_name, row_idx, col_idx);

// Shows dialog and returns result
if let Some((new_value, row_idx, col_idx)) = edit_dialog.show(ctx) {
    // Save changes
}
```

### 2. update_cell (db.rs)

```rust
pub async fn update_cell(
    &self,
    schema: &str,
    table: &str,
    column: &str,
    new_value: &str,
    row_data: &[String],
    columns: &[String],
) -> Result<()>
```

**Steps:**
1. Detect primary key from pg_index
2. Find primary key value in row data
3. Build parameterized UPDATE query
4. Execute query
5. Return success or error

### 3. ResultsTable Changes

```rust
// Now returns clicked cell info
pub fn show(&mut self, ui: &mut Ui) 
    -> Option<(String, String, usize, usize)>

// Update cell in UI
pub fn update_cell(&mut self, row_idx: usize, col_idx: usize, new_value: String)
```

### 4. App Integration

```rust
// In update() method
if let Some((value, column_name, row_idx, col_idx)) = 
    self.results_table.show(ui) {
    self.edit_dialog.open(value, column_name, row_idx, col_idx);
}

// Handle edit dialog result
if let Some((new_value, row_idx, col_idx)) = self.edit_dialog.show(ctx) {
    // Send update command
    self.update_cell(schema, table, column, new_value, ...);
    
    // Update UI immediately
    self.results_table.update_cell(row_idx, col_idx, new_value);
}
```

## Safety

### SQL Injection Prevention

✅ **Safe:** Uses parameterized queries
```rust
client.execute(&query, &[&new_value, &pk_value]).await?;
```

❌ **Unsafe (we DON'T do this):**
```rust
let query = format!("UPDATE table SET col = '{}' WHERE id = '{}'", value, id);
```

### Validation

- Non-empty primary key required
- Column must exist
- Connection must be active
- Proper error handling

## Limitations

### Current Version

1. **Edits single cells only** - no batch editing yet
2. **Requires primary key or first column** - may fail on tables without proper keys
3. **No type validation** - can enter invalid data (database will reject)
4. **No undo** - changes are immediate (use transactions in SQL for safety)
5. **Table name/column name sanitization** - trusts schema metadata

### Workarounds

**No primary key:**
- Ensure first column uniquely identifies rows
- Or use SQL editor for complex updates

**Type validation:**
- Database will reject invalid types
- Error shown in status bar

**Undo:**
- Use SQL transactions manually if needed
- Take backups before bulk edits

## Examples

### Edit Email

```
1. View users table
2. Double-click email cell
3. Change: alice@old.com → alice@new.com
4. Click Save
5. ✅ Updated!
```

### Edit Price

```
1. View products table
2. Double-click price cell
3. Change: 19.99 → 24.99
4. Click Save
5. ✅ Price updated!
```

### Edit Status

```
1. View orders table
2. Double-click status cell
3. Change: pending → completed
4. Click Save
5. ✅ Status changed!
```

## Testing

```bash
# Build
cargo build --release

# Test
1. Connect to PostgreSQL
2. SELECT * FROM any_table;
3. Double-click a cell
4. Edit value
5. Save
6. Verify:
   - Dialog closes
   - Table reloads
   - New value visible
   - Status: "Cell updated successfully"
```

## Future Enhancements

Planned for future versions:

- [ ] Batch editing (multiple cells)
- [ ] Type validation before save
- [ ] Undo/Redo functionality
- [ ] Edit history
- [ ] Confirmation dialog for changes
- [ ] Support for NULL values
- [ ] Date/time pickers for temporal types
- [ ] Dropdown for ENUM types
- [ ] Foreign key lookups

## Performance

- ✅ Immediate UI update (optimistic)
- ✅ Background database update
- ✅ Automatic reload on success
- ✅ No blocking of UI thread

## Troubleshooting

### "No primary key value" error

**Cause:** Table has no primary key and first column isn't unique

**Solution:** 
- Add primary key to table
- Or use SQL editor for updates

### "Cell updated successfully" but value unchanged

**Cause:** Database rejected update (type mismatch, constraint violation)

**Solution:**
- Check data type
- Check constraints
- View PostgreSQL logs for details

### Can't find the cell I edited

**Cause:** Table has multiple pages, edited on different page

**Solution:**
- Use pagination to find the row
- Or use SQL: SELECT * FROM table WHERE column = 'value';

---

**Version**: 0.2.0  
**Date**: 2024-12-09  
**Feature**: Cell Editing  
**Status**: ✅ Production Ready  
**Type**: Major Feature Release
