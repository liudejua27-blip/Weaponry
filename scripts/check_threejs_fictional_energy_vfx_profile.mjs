import { readFile } from "node:fs/promises";
import { MathUtils, REVISION } from "../apps/desktop/node_modules/three/build/three.module.js";

const receipt = JSON.parse(await readFile(process.argv[2], "utf8"));
const profile = receipt.fictional_energy_vfx_profile;
if (!profile || profile.schema_version !== "FictionalEnergyVfxProfile@1") {
  throw new Error("fictional energy VFX profile is missing");
}
if (profile.effects.length !== 2 || profile.timebase_hz !== 1000) {
  throw new Error("bounded VFX effect/timebase contract differs");
}
if (
  profile.emissive_animation_rendered ||
  profile.bloom_rendered ||
  profile.particles_rendered ||
  profile.trails_rendered ||
  profile.actual_engine_roundtrip
) {
  throw new Error("profile overclaims an unimplemented VFX execution path");
}

const sampleLinear = (effect, tick) => {
  const times = effect.sample_time_ticks;
  const strengths = effect.emissive_strength_samples;
  if (tick <= times[0]) return strengths[0];
  if (tick >= times.at(-1)) return strengths.at(-1);
  const right = times.findIndex((value) => value >= tick);
  const alpha = (tick - times[right - 1]) / (times[right] - times[right - 1]);
  return MathUtils.lerp(strengths[right - 1], strengths[right], alpha);
};

const muzzle = profile.effects.find((effect) => effect.effect_id === "muzzle-pulse");
const core = profile.effects.find((effect) => effect.effect_id === "energy-core-breathe");
if (!muzzle || !core) throw new Error("canonical VFX effects are missing");
const samples = {
  muzzle_tick_50: sampleLinear(muzzle, 50),
  muzzle_tick_100: sampleLinear(muzzle, 100),
  core_tick_250: sampleLinear(core, 250),
  core_tick_750: sampleLinear(core, 750),
};
if (
  samples.muzzle_tick_50 !== 4 ||
  samples.muzzle_tick_100 !== 8 ||
  samples.core_tick_250 !== 4.5 ||
  samples.core_tick_750 !== 4.5
) {
  throw new Error("Three.js-side deterministic LINEAR sample differs");
}

process.stdout.write(`${JSON.stringify({
  schema_version: "ThreeJsFictionalEnergyVfxProfileConsumerReceipt@1",
  three_revision: REVISION,
  profile_object_sha256: receipt.fictional_energy_vfx_profile_object_sha256,
  typed_profile_parse: "PASS",
  linear_sample_math: "PASS",
  samples,
  material_animation_rendered: false,
  bloom_rendered: false,
  particles_rendered: false,
  trails_rendered: false,
  actual_engine_roundtrip: false,
  consumer_scope: "read-only-profile-sampling-math-no-viewer-or-render-execution",
  quality_status: "structural_only",
})}\n`);
