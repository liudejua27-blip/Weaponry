# Constraints

- No network, arbitrary filesystem path, environment variable, secret, model call, shell, Python or JavaScript.
- Units are metres and the coordinate system is right-handed Y-up.
- Operators are selected only from the checked-in lock and are implemented by ForgeCAD.
- Invalid DAGs, non-finite values, unknown operators and budget overflow fail closed.
- The MCP adapter accepts only `skill_get` for `ponytail-preflight@0.1.0` before other ForgeCAD design tools or Skills in a session; the bootstrap diagnostics `capabilities_get`, `runtime_status` and `doctor` remain read-only exemptions.
- This preflight does not authorize a geometry claim or a persistent write. Use the existing typed prepare, readback, quality and user-confirm steps, and retain unknown or occluded reference evidence as unknown.
- Do not install or execute the upstream Node package, its hooks, its MCP server, or arbitrary repository files.
