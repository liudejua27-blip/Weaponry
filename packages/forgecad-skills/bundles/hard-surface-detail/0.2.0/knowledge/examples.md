# Synthetic examples

The fixture for `hard-surface-detail@0.2.0` is deliberately synthetic and contains no user image, model weight or external asset. It is used only to exercise declarative validation.

The planning order used by the current recipe is:

`profile-body → profile-shell → panel-shell → vent-grid → part-sink`

with the parallel mechanical branch:

`profile-body → revolve-joint → joint-stack → cable-sweep → mirror-limb → array-ribs → transform-fit → part-sink`.

This is a bounded recipe DAG. It does not assert that every stage is visible
in a single reference image: Codex must label each region `observed`,
`inferred` or `unknown` before choosing dimensions.
