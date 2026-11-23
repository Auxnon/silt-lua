# TODO

## `pairs` over a large hash part is quadratic

`Table::next_entry` (src/table.rs) resumes a traversal by scanning the hash part
from the start for the previous key, so `next` is O(n) and a full `pairs` walk
over n hash keys is O(n²). 50k keys took over a minute in a debug build
(`tests/table_memory.rs` keeps its `pairs` test at 5k keys for this reason). The
array part is unaffected.

Options:
- **`indexmap`** for the hash part: `get_index_of(key)` gives the position in
  O(1) and iteration is by index. Dead keys (`t[k] = nil` keeps the key) mean no
  removals mid-traversal, so the order stays stable. It touches every table
  lookup (an extra indirection), so it needs the full benchmark A/B.
- hashbrown's `raw` feature (`RawTable::find` → bucket index, resume from
  there): fast, but needs `unsafe` bucket access, which the project avoids.
- A cursor cached on the table (last key + position) for the common
  one-traversal-at-a-time case, falling back to the scan otherwise.
