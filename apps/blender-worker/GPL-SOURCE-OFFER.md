# Blender GPL source acquisition record

This file is shipped with the fixed Blender sidecar as a development packaging
record. It is an acquisition description, not legal advice and not a completed written offer. The product/legal owner must review and replace or approve it
before any external distribution.

## Covered binary

| Field | Value |
| --- | --- |
| Component | Blender |
| Version | 5.2.1 LTS |
| Platform | macOS arm64 |
| Build branch | `blender-v5.2-release` |
| Build hash recorded by Blender | `9e2066aef7ef` |
| SPDX license | `GPL-3.0-or-later` |
| Bundle identifier | `org.blenderfoundation.blender` |
| Executable SHA-256 | `ea651e507c6b197df0e234bfa04e5ed43e7f4d498267a7df93fcb38f21928a5c` |
| Complete bundle tree SHA-256 | `a1719f3e1c7fc846e811de3c9d32ff72f2130016a3290fa88eb4c8e9e1032317` |

The build hash is the identity emitted by the executable in the staged bundle.
It is not silently promoted to a full source commit or to a downloadable
archive digest. The full source/release identity and any applicable offer
period still require an independent product/legal decision.

## Official source locations

Use the Blender project's official source and release channels:

1. Source repository: <https://github.com/blender/blender>
2. Release downloads: <https://www.blender.org/download/releases/>
3. License guidance: <https://www.blender.org/about/license/>

The source checkout or archive supplied for a distribution must be the one that
corresponds to the covered 5.2.1 build and must retain Blender's `COPYING`,
license directory, third-party notices, and build metadata. Do not substitute a
nearby nightly, a different architecture, or a different Blender release.

## Verification procedure

After obtaining the source through an approved official channel:

1. Record the complete upstream commit or release archive digest; the current
   workspace has only the executable's 12-character build hash above.
2. Confirm that the source build reports Blender 5.2.1 and build hash
   `9e2066aef7ef` with the same `blender-v5.2-release` branch identity.
3. Preserve the source tree and all corresponding license/third-party files
   alongside the distributed application or at the approved source-offer
   location.
4. Recompute the executable, bundle-tree, and license-resource hashes and
   compare them with `apps/blender-worker/manifest.json` and the packaged
   manifest. Any mismatch is a fail-closed packaging error.

No source archive is present in this workspace or in the staged resource. The
current package only carries this reproducible acquisition description plus
Blender's in-bundle license texts. Until the product/legal owner validates the
source path, corresponding-source availability, and offer wording, the release
gate must remain `NOT_RUN`/`NOT_PROVEN`.
