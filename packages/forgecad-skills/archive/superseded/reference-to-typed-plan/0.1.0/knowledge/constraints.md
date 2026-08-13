# Constraints

- Only a `ReferenceEvidence` ID and its CAS hash are accepted; never a local path or raw image bytes.
- Every claim must identify a source region and confidence (`observed`, `inferred`, or `unknown`).
- Hidden surfaces are `unknown` or low-confidence inference; they are never promoted to observed truth.
- Part IDs and MaterialZone IDs are stable opaque identifiers, not display labels.
- Units and coordinate conventions are explicit; an image alone does not establish physical scale.
- Detail inventory items must map to a typed operator family later, or be marked `unrealized`.
- `img2threejs` code/Three.js output and `img2css` CSS output are reference inspiration only.
- No recipe node may perform I/O, network access, dynamic evaluation, or model invocation.
