# Example route

For a three-quarter robot, begin with stable Parts such as head-shell, visor,
neck, chest-shell, chest-core, chest-panel, shoulders, arms, pelvis and thighs.
Use a second source node for a chest detail only when the semantic sink lists
both source IDs in deterministic order. Iterate one Part at a time and call
`quality_get` after each structural change. If the report is `limited` or a
visual gate is unavailable, stop the visual-quality route: do not describe the
candidate as high quality, PBR-ready, reference-matched or 360-complete. Only
continue to structural `candidate_confirm`/`export_confirm` when the user
explicitly chooses an MVP `STRUCTURAL_BLOCKOUT` result and the same candidate
still passes all Runtime geometry/readback/approval gates; record that
limitation in the receipt.
