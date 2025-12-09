# Type-Aware Editing v0.2.1

## New Feature: Smart Type Detection! 🎯

Showel теперь определяет типы данных колонок и предоставляет специализированные элементы управления для редактирования!

## Features

### 1. Boolean Type Support

**Для boolean полей:**
- Кнопки выбора вместо текстового ввода
- ✓ true / ✗ false / NULL
- Визуальные индикаторы
- Один клик для изменения

**UI:**
```
┌─────────────────────────────────────┐
│  Edit: is_active                    │
│  Type: boolean                      │
├─────────────────────────────────────┤
│  Original: true                     │
│  ─────────────────────────────────  │
│  New value:                         │
│  [✓ true] [✗ false] [NULL]         │
│                                     │
│  ⚠ Value will be updated            │
│  [Save]  [Cancel]                   │
└─────────────────────────────────────┘
```

### 2. Numeric Type Support

**Для числовых полей:**
- Валидация ввода
- Предупреждение при некорректном числе
- Подсказка "Enter number..."
- Кнопка "Set NULL"

**UI:**
```
┌─────────────────────────────────────┐
│  Edit: price                        │
│  Type: numeric                      │
├─────────────────────────────────────┤
│  Original: 19.99                    │
│  ─────────────────────────────────  │
│  New value:        [Set NULL]       │
│  [24.99________________]            │
│                                     │
│  [Save]  [Cancel]                   │
└─────────────────────────────────────┘
```

**С ошибкой:**
```
│  New value:        [Set NULL]       │
│  [abc_________________]             │
│  ⚠ Invalid number                   │
```

### 3. Text Type Support

**Для текстовых полей:**
- Обычный текстовый ввод
- Кнопка "Set NULL"
- Без ограничений на содержимое

### 4. NULL Value Support

**Для всех типов:**
- Кнопка "Set NULL" (текст/числа)
- Опция "NULL" (boolean)
- Правильная обработка в SQL

## Supported Types

### Boolean Types
- `boolean`
- `bool`

**UI:** Toggle buttons (✓ true / ✗ false / NULL)

### Numeric Types
- `int2`, `smallint`
- `int4`, `integer`
- `int8`, `bigint`
- `numeric`, `decimal`
- `real`, `float4`
- `double precision`, `float8`

**UI:** Text input with validation

### Text Types
- `varchar`, `text`
- `char`, `bpchar`
- All other types (default)

**UI:** Text input

## How It Works

### 1. Type Detection

When you open a table, Showel queries column types:

```sql
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = $1 AND table_name = $2
ORDER BY ordinal_position
```

Результат сохраняется в `column_types: Vec<(String, String)>`

### 2. Edit Dialog

При открытии диалога редактирования:

```rust
// Получить тип колонки
let column_type = column_types.iter()
    .find(|(col, _)| col == &column_name)
    .map(|(_, typ)| typ.clone())
    .unwrap_or_else(|| "text".to_string());

// Открыть диалог с типом
edit_dialog.open(value, column_name, row_idx, col_idx, column_type);
```

### 3. Type-Specific UI

В `EditDialog::show()`:

```rust
let type_lower = self.column_type.to_lowercase();

if type_lower == "boolean" || type_lower == "bool" {
    // Показать кнопки true/false/NULL
    ui.selectable_label(value == "true", "✓ true");
    ui.selectable_label(value == "false", "✗ false");
    ui.selectable_label(value == "NULL", "NULL");
} else if is_numeric(type_lower) {
    // Показать текстовое поле с валидацией
    TextEdit::singleline(&mut value)
        .hint_text("Enter number...");
    
    // Проверить валидность
    if value.parse::<f64>().is_err() {
        ui.colored_label(RED, "⚠ Invalid number");
    }
} else {
    // Обычный текстовый ввод
    TextEdit::singleline(&mut value);
}
```

### 4. NULL Handling

В UPDATE запросе:

```rust
if new_value.to_uppercase() == "NULL" {
    // UPDATE table SET column = NULL WHERE pk = $1
    client.execute(&query, &[&pk_value]).await?;
} else {
    // UPDATE table SET column = $1 WHERE pk = $2
    client.execute(&query, &[&new_value, &pk_value]).await?;
}
```

## Examples

### Edit Boolean Field

```
1. Table: users, Column: is_active (boolean)
2. Current value: true
3. Double-click cell
4. Click "✗ false" button
5. Save
6. ✅ Updated to false
```

### Edit Numeric Field

```
1. Table: products, Column: price (numeric)
2. Current value: 19.99
3. Double-click cell
4. Enter: 24.99
5. Save
6. ✅ Price updated
```

### Set NULL

**For boolean:**
```
1. Double-click boolean cell
2. Click "NULL" button
3. Save
4. ✅ Set to NULL
```

**For text/numeric:**
```
1. Double-click cell
2. Click "Set NULL" button
3. Save
4. ✅ Set to NULL
```

### Invalid Input

```
1. Double-click numeric cell (e.g., quantity)
2. Enter: "abc"
3. See: "⚠ Invalid number" warning
4. Cannot save until corrected
5. Enter: "123"
6. Warning disappears
7. Save → ✅
```

## Technical Implementation

### Database Layer

```rust
// db.rs
pub async fn get_column_types(
    &self,
    schema: &str,
    table: &str,
) -> Result<Vec<(String, String)>> {
    // Query information_schema.columns
    // Return Vec of (column_name, data_type)
}
```

### Command/Response

```rust
// app.rs
enum DbCommand {
    GetColumnTypes(String, String),
    // ...
}

enum DbResponse {
    ColumnTypes(Vec<(String, String)>),
    // ...
}
```

### UI Layer

```rust
// ui.rs
pub struct EditDialog {
    pub column_type: String,  // New field
    // ...
}

impl EditDialog {
    pub fn open(
        &mut self,
        value: String,
        column_name: String,
        row_index: usize,
        col_index: usize,
        column_type: String,  // New parameter
    ) {
        self.column_type = column_type;
        // ...
    }
}
```

### Type Detection Logic

```rust
let is_bool = matches!(
    type_lower.as_str(),
    "bool" | "boolean"
);

let is_numeric = matches!(
    type_lower.as_str(),
    "int2" | "int4" | "int8" | "integer" | 
    "smallint" | "bigint" | "numeric" | "decimal" |
    "real" | "double precision" | "float4" | "float8"
);
```

## Benefits

### User Experience
- ✅ Easier boolean editing (click vs type)
- ✅ Prevents invalid input (numeric validation)
- ✅ Clear visual feedback
- ✅ Type information visible
- ✅ NULL handling simplified

### Data Integrity
- ✅ Type validation prevents errors
- ✅ Boolean values always correct
- ✅ Numeric validation catches typos
- ✅ Proper NULL handling

### Usability
- ✅ No need to remember type syntax
- ✅ Clear visual indicators (✓/✗)
- ✅ Immediate validation feedback
- ✅ One-click boolean changes

## Limitations

### Current Version

1. **Limited type support** - Only bool, numeric, text
2. **No date pickers** - Dates entered as text
3. **No enum dropdowns** - Enums entered as text
4. **No JSON editor** - JSON as text
5. **Basic validation** - Only for numeric types

### Future Enhancements

Planned for future versions:

- [ ] Date/time picker for temporal types
- [ ] Dropdown for ENUM types
- [ ] JSON editor with syntax highlighting
- [ ] Array editor for array types
- [ ] Foreign key lookup/autocomplete
- [ ] UUID generator button
- [ ] Color picker for color types
- [ ] Custom validators per type

## Troubleshooting

### Type not detected

**Cause:** Uncommon or custom type

**Solution:** Falls back to text input, edit as string

### Validation too strict

**Cause:** Numeric validation

**Solution:** 
- Enter valid number
- Or use SQL editor for special cases

### NULL not working

**Cause:** Column is NOT NULL

**Solution:** PostgreSQL will reject, check constraints

## Testing

```bash
# Build
cargo build --release

# Test Boolean
1. Find table with boolean column
2. Double-click boolean cell
3. Verify: Three buttons (✓ true, ✗ false, NULL)
4. Click each, verify selection
5. Save, verify update

# Test Numeric
1. Find table with numeric column
2. Double-click cell
3. Type invalid: "abc"
4. Verify: "⚠ Invalid number" appears
5. Type valid: "123"
6. Verify: Warning disappears
7. Save, verify update

# Test NULL
1. Double-click any cell
2. Click "Set NULL" or "NULL"
3. Save
4. Verify: Cell shows NULL (if nullable)
```

## Code Changes

### Files Modified

```
src/db.rs       328 → 357 строк  (+29)
src/app.rs      504 → 530 строк  (+26)
src/ui.rs       457 → 511 строк  (+54)
Cargo.toml      v0.2.0 → v0.2.1
CHANGELOG.md    Updated
```

### New Methods

- `get_column_types()` - Query column types from database
- Type detection logic in EditDialog
- NULL handling in UPDATE queries

---

**Version**: 0.2.1  
**Date**: 2024-12-09  
**Feature**: Type-Aware Editing  
**Status**: ✅ Production Ready  
**Type**: Minor Feature Release
