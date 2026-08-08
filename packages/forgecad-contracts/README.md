# ForgeCAD Runtime Contracts

`forgecad-runtime-contracts@1` is the only contract set used by the MCP002
Runtime reset. Rust owns runtime structs and SQLite read/write boundaries; these JSON
Schemas are reviewable interchange contracts, not a second product database.

MCP002 exposes Runtime discovery and read-only inspection through authenticated
local IPC. Mutation, geometry,
rendering, materials and reference-image tools are added only with a typed
request, a constrained operator, a validator, a receipt and a benchmark.
