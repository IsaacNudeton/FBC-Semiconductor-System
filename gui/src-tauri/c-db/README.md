# LRM C Database Engine — Shipping Today

**Copied from:** `Lab-Resource-Manager-v2-Isaac-/c-engine/src/`  
**Location:** `FBC-Semiconductor-System/gui/src-tauri/c-db/`

## Files to Copy

Copy these files from LRM to `c-db/`:

```
page.c          ← 4KB page management, buffer pool
page.h
btree.c         ← B-tree index implementation
btree.h
wal.c           ← Write-ahead log
wal.h
table.c         ← Table layer
table.h
schema.c        ← Schema initialization (21 tables)
schema.h
lrm_db.h        ← Main header
inventory.c     ← Burn-in inventory queries
tracker.c       ← Burn-in tracker
lrm_ffi.c       ← Rust FFI wrapper (created today)
lrm_ffi.h       ← FFI header (created today)
```

## Build

```bash
cd gui/src-tauri
cargo build
```

The `build.rs` will compile all C files and link them statically.

## Usage in Rust

```rust
use crate::database::lrm_ffi::Database;

let db = Database::open("lrm_inventory.db")?;
db.init_schema()?;

let controllers = db.get_controllers()?;  /* JSON result */
let lots = db.get_lots()?;
```

---

**This is the custom C engine. 4KB pages. B-tree indexes. WAL. Ships today.** 🔥
