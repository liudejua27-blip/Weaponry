# ForgeCAD Runtime Contracts

`forgecad-runtime-contracts@1` is the only contract set used by the MCP002 Runtime
reset, MCP004 transaction core, and MCP005 reference admission. The manifest currently
contains 27 schemas. Rust owns runtime structs and SQLite read/write boundaries; these
JSON Schemas are reviewable interchange contracts, not a second product database.

MCP003 exposes Runtime discovery and read-only inspection through the MCP stdio
adapter. MCP004 adds typed candidate/approval/restore/diagnostic-export transaction records behind
authenticated local IPC; MCP005 adds bounded PNG/JPEG reference admission and
hash-bound `ReferenceEvidence` readback. Geometry, rendering, materials and Skill
execution remain unavailable until each has a constrained operator, validator,
receipt and benchmark.
