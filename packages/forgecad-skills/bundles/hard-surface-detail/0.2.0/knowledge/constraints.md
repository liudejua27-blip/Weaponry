# Constraints

- No network, arbitrary filesystem path, environment variable, secret, model call, shell, Python or JavaScript.
- Units are metres and the coordinate system is right-handed Y-up.
- Operators are selected only from the checked-in lock and are implemented by ForgeCAD.
- Invalid DAGs, non-finite values, unknown operators and budget overflow fail closed.
- Keep the recipe order stable, but emit one canonical V2 program for each
  candidate; do not encode a script, free-form expression, path or URL in a
  node.
- A one-view robot may use inferred symmetry and hidden structure only when it
  is explicitly marked inferred/unknown; it cannot claim a 360-degree result.
- `part_id`, `source_node_id` and `material_zone_id` must survive strict GLB
  readback before the candidate can enter visual review.
