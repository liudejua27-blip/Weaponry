# Dragonfang Kukri r8 — Three.js delivery

This directory is a geometry-frozen, action-ready delivery of the accepted r8 approximation. It is not a commercial-art, human-review, Unreal, animation, UV/bake, or visual-quality PASS.

## Load

The standalone module already contains the pinned Three.js runtime and GLTFLoader:

```js
import { loadKnifeDelivery } from './load-knife-delivery.standalone.mjs'

const { root, controller, manifest } = await loadKnifeDelivery({
  baseUrl: new URL('./', import.meta.url),
})
scene.add(root)
controller.setExploded(0.6)
controller.setPartVisible('relief-dragon-spine', false)
```

Raycast hits can be resolved with `controller.resolvePart(hit.object)`. The package contains 13 stable part pivots, three named sockets, two collider intents and two destruction groups.

The original source GLB is retained under `provenance/` for geometry-byte comparison. The 48 fixed-view PNG/AOV files are retained under `evidence/fixed-views/`; the authorized reference image is not bundled.
