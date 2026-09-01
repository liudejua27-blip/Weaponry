# img2threejs pinned adoption metadata

This is an isolated-cache receipt for the img2threejs source baseline used by
the Weaponry Three.js knife route. It is not an npm dependency, a Runtime
plugin, a vendored source tree, or an executable upstream skill.

Verify the repository metadata without network access:

    python3 verify_adoption.py

Restore the exact upstream commit into a cache outside the product tree:

    python3 restore_pinned_snapshot.py --cache-root /absolute/path/to/cache

Verify an existing cache checkout:

    python3 restore_pinned_snapshot.py --verify /absolute/path/to/cache/img2threejs/9fbd0ca5bbcc3b13bebe712745d6784d33db0b85

The restore helper is maintenance-only. It may fetch the fixed upstream commit,
but it never runs upstream scripts and it refuses to use the product tree as a
cache root. The first closed static ground-blade import now lives in
`packages/weaponry-threejs/src/img2threejs-object-sculpt-adapter.ts`. It reads
bounded ObjectSculptSpec data and never executes upstream code. Worker isolation,
malicious-input benchmarking, browser quality calibration and full component
coverage remain pending.
