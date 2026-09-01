/** Bounded ObjectSculptSpec-shaped fixture matching the pinned upstream ground-blade descriptor. */
export const img2threejsGroundBladeFixture = {
  targetName: 'Pinned Baseline Kukri',
  schemaVersion: '2.1',
  coordinateFrame: { up: '+Y', forward: '+Z', units: 'normalized design units' },
  materials: [
    { id: 'skin-finish', baseColor: '#78232d', metalness: { base: 0.82 }, roughness: { base: 0.3 } },
    { id: 'substrate', baseColor: '#b9852e', metalness: { base: 1 }, roughness: { base: 0.22 } },
  ],
  componentTree: [
    { id: 'root', role: 'body', primitive: 'box', material: 'skin-finish' },
    {
      id: 'blade',
      role: 'blade',
      primitive: 'ground-blade',
      material: 'skin-finish',
      geometryDescriptor: {
        bladeSpec: {
          stations: [
            [0, 0.08, -0.09],
            [0.12, 0.086, -0.1],
            [0.3, 0.086, -0.11],
            [0.5, 0.084, -0.108],
            [0.63, 0.078, -0.095],
            [0.74, 0.055, -0.055],
            [0.82, 0.028, -0.02],
            [0.88, 0, -0.001],
          ],
          thickness: 0.05,
          grindFrac: 0.55,
          swedgeFromTipFrac: 0.34,
        },
      },
    },
    { id: 'grip', role: 'grip', primitive: 'curve-sweep', material: 'skin-finish' },
  ],
} as const
