# Example shape

The example below is intentionally incomplete. It demonstrates uncertainty and
stable IDs without claiming that a robot reference has been reconstructed:

```json
{
  "reference_id": "reference-example",
  "reference_sha256": "<64 lowercase hex>",
  "subject_class": "humanoid-mechanical",
  "observations": [
    {"id":"obs-silhouette","kind":"silhouette","region":"full-frame","confidence":0.91,"claim":"two-arm humanoid outline"},
    {"id":"obs-hidden-back","kind":"unknown","region":"back","confidence":0.12,"claim":"not visible in source image"}
  ],
  "parts": [
    {"part_id":"part-torso","parent_id":null,"role":"torso-shell","symmetry":"centered"}
  ],
  "material_zones": [
    {"zone_id":"zone-white-shell","part_id":"part-torso","finish":"painted-metal","source":"sampled-reference"}
  ],
  "open_questions": ["physical scale", "rear geometry", "joint articulation"]
}
```
