# Pagination Implementation Plan

## Current State
- Application has virtual scrolling implemented
- Loads data incrementally as user scrolls
- Shows "Loading..." indicators
- No pagination controls

## Desired State
- Add traditional pagination controls (Previous/Next buttons)
- Keep virtual scrolling as an option
- Allow users to toggle between modes
- Add page size selector (50, 100, 200 rows)

## Implementation Approach

### 1. Add Pagination Fields to ShowelApp
```rust
// Add these fields:
table_page_size: i64,      // Rows per page (50, 100, 200)
use_pagination: bool,     // Toggle between pagination and virtual scrolling
```

### 2. Modify load_table_data Function
- Reset table_page to 0 when loading new table
- Call appropriate loading function based on mode

### 3. Add load_paginated_data Function
- Calculate offset based on current page
- Load exactly one page of data
- Replace all rows (don't append)

### 4. Update TableData Response Handling
- Handle both pagination and virtual scrolling modes
- Different status messages for each mode

### 5. Add Pagination UI Controls
- Toggle checkbox for mode selection
- Previous/Next buttons
- Page counter ("Page X of Y")
- Page size selector

### 6. Update ResultsTable for Pagination Mode
- Disable virtual scrolling indicators when in pagination mode
- Show only current page of data

## Key Changes Needed

1. **src/app.rs**:
   - Add pagination fields
   - Add load_paginated_data function
   - Update TableData response handling
   - Add pagination UI controls

2. **src/ui.rs**:
   - Modify ResultsTable to work with pagination mode
   - Disable loading indicators when in pagination mode

## Benefits
- Users can choose their preferred navigation method
- Pagination is familiar for database applications
- Virtual scrolling is better for large datasets
- Both approaches work with the same backend

## Implementation Steps
1. Add fields to ShowelApp struct
2. Initialize fields in constructor
3. Add load_paginated_data function
4. Update load_table_data to support both modes
5. Update TableData response handling
6. Add pagination UI controls
7. Test both modes

This approach maintains backward compatibility while adding the requested pagination functionality.