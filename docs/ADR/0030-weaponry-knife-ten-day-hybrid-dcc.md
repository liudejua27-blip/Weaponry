# ADR-0030: Weaponry knife-first ten-day hybrid DCC

- Status: accepted for implementation
- Date: 2026-08-29
- Supersedes: ADR-0029 的首月品类优先级，以及 ADR-0027/0028 中“Blender 只能作为研究参考”的绝对禁令
- Preserves: Runtime 单写者、closed contracts、Store/CAS、确定性回放、显式批准、历史 evidence 不可改写

## Decision

Weaponry 的第一个商业交付品类收缩为穿越火线刀类视觉资产。十天目标不是让
BMesh、Sculpt、NURBS、Geometry Nodes、Retopology、UV、Bake、动画和插件生态获得
Blender 的通用品类成熟度，而是让这些能力对刀类生产任务达到足够覆盖：刀身与护手
硬表面、局部雕刻/倒角、可编辑低模、Hero UV、High→Low Cage Bake、金属/涂层/磨损
材质、第一人称持刀与检视动作，以及目标引擎交付。

实现采用双轨：

1. Rust-native 产品轨负责授权、AuthoringMesh、稳定 ID、事务、Modifier/Evaluation Graph、
   Store/CAS、版本、质量、回读、审批和最终交付真值。
2. 固定版本的 Blender 与固定插件集合可以作为隔离的内部高能力原型执行器。它只接受
   closed typed job，不接受 Codex/用户传入 Python、路径、URL 或任意 add-on；输出必须由
   Rust 重新解析、验证、hash、绑定 lineage 后才能成为候选派生物。

`.blend`、Blender 会话、插件状态和 Python 对象不成为 Weaponry 产品真值。内部原型失败、
超时、版本漂移或许可证不明时必须 fail closed，不能静默切换或伪造 Rust-native PASS。

## Fixed revision and capability/provider boundary

The only currently named Blender source snapshot is
[`72ccdd6e96ca119a1ffa3372559cc5654343b477`](https://github.com/blender/blender/commit/72ccdd6e96ca119a1ffa3372559cc5654343b477).
The revision is an upstream commit rather than a release tag; its
[`BKE_blender_version.h`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/blenkernel/BKE_blender_version.h)
reports Blender `5.3.0-alpha`. It must therefore be pinned with its build flags, compiler,
Python bundle and dependency SBOM; Blender 4.5 LTS documentation is workflow reference, not
an exact compatibility promise.

The source [`COPYING`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/COPYING)
states GPL for Blender and no alternative Blender-wide license. The official
[`license guidance`](https://www.blender.org/about/license/) says published Python add-ons
must be GPL-compatible while artwork/data rights are separate. A process boundary is not a
license exemption: no Blender or plugin source, Python, `.blend`, brush, decal, rig or other
asset is copied or linked into Weaponry without a separate legal decision. The fixed source is
also a C/C++/Python system with optional dependencies and headless/background switches, not a
small embeddable Rust library; see its [`CMake`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/CMakeLists.txt),
[`BMesh`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/bmesh/CMakeLists.txt),
[`Depsgraph`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/depsgraph/CMakeLists.txt)
and [`Nodes`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/nodes/CMakeLists.txt)
source manifests.

For the ten-day knife vertical slice, Blender built-ins may answer workflow questions for
BMesh/hard-surface, bounded Multires/Sculpt, Quad Remesh/retopology drafts, UV/packing,
Cycles cage bake, Principled materials, fixed FPS cameras, and rigid Action/F-Curve/NLA clips.
Geometry Nodes and NURBS remain reference-only except for a closed typed knife subset. The
product-owned side is non-negotiable: stable-ID `AuthoringMesh`, editable Low and High/Low
correspondence, cage/bake diagnostics and map readback, `MaterialLayerGraph`, FPS camera/socket
package, rigid animation clips/events, deterministic hashes, Runtime CAS/Store, approval and
engine readback. Blender output is only a `PrototypeObservation` and cannot advance Stage,
confirm, version or export.

Provider decisions are deliberately narrow:

- Node Wrangler and Rigify are bundled/official GPL add-ons whose fixed source imports `bpy`;
  Magic UV is an official GPL-2.0-or-later Extension. They can inform material, UV and rigid
  rig workflows only, subject to exact host validation.
- RetopoFlow has a manifest marked GPL-2.0-or-later but its official docs describe code as
  GPL 3.0 and non-code assets separately; the conflict blocks adoption.
- UVPackmaster separates a GPL addon from a separately licensed EULA engine; the engine and
  installer cannot enter the product. Zen UV Checker is GPL-3.0-or-later and requests Files
  permission, and is only a checker.
- MESHmachine, HardOps/Boxcutter and DECALmachine are vendor/commercial products without a
  product redistribution grant in this audit; DECALmachine's current supported host stops at
  Blender 5.1 while the pinned source is 5.3 alpha. They are not providers for the fixed host.

Every future provider must use a closed typed job and offline scratch, pin exact source/plugin
hashes and permissions, and declare input/output, resource limits, headless command, two-run
determinism and removal receipt. `-b/--background` is only UI-less command-line operation, not
proof that Python/plugins are safe or that bytes are cross-platform deterministic. Input is an
authorized fixture and typed parameters; output is temporary GLB/FBX, maps/AOVs, clips and a
content-free receipt. Runtime independently checks semantic geometry, UV/tangent/PBR/socket/
animation readback. Any license, host, resource or replay failure is `NOT_PROVEN`/removed, never
a silent fallback.

The required migration ladder is: (1) license/scope classification; (2) operator/workflow
behavior inventory; (3) ForgeCAD schema, limits, errors and provenance; (4) replacement of
data-block/BMesh/context handles by stable-ID product data; (5) pure Rust algorithm and bounded
typed graph; (6) preview/prepare/commit/rollback transaction; (7) deterministic differential,
negative/resource and strict readback tests; (8) SBOM/signature/removal receipt and provider
shutdown after the Rust fixture/human/engine gates pass. This is semantic reimplementation,
not translation of plugin source. No entire Blender checkout, binary, Python bundle, add-on zip,
installation script, socket/BlenderMCP, dynamic plugin loader, `.blend` truth or direct CAS/DB
write is allowed.

The detailed weapon matrix, official URLs and plugin-specific exit rules are recorded in
[`EXTERNAL_PROJECT_ADOPTION.md`](../EXTERNAL_PROJECT_ADOPTION.md#61-blender-fixed-revision-and-weapon-prototype-boundary-2026-08-29).

## First-principles correction

“刀类比枪械简单”只在部件数量、机械装配、附件和部分动画维度成立。刀刃轮廓、曲面连续性、
高低模投射、锋线高光、材质磨损和第一人称近景会放大任何法线、UV、烘焙和 shading 缺陷。
因此工程量确实小于完整枪械，但不等于可以省略 artist-editable Low、Cage 诊断、切线一致性、
独立人审或真实引擎验证。

“下载成熟插件后转成 Rust”也不是机械翻译。多数 Blender 插件依赖 `bpy`、BMesh、Depsgraph、
operator context、data-block、undo 和 Blender Python 宿主。迁移必须拆成可观察语义、固定 fixture、
隔离原型、差分测试、Rust 重实现和 provider 切换；直接复制 GPL Blender/插件代码到非 GPL
Rust 产品可能产生许可证义务，必须逐仓库审计。

## Knife task maturity matrix

| Blender-inspired family | Ten-day knife requirement | Implementation lane |
| --- | --- | --- |
| BMesh | stable-ID split/extrude/inset/bevel/bridge/dissolve/merge/loop operations | Rust product truth |
| Sculpt | crease/smooth/flatten/inflate-like bounded surface pass and mask by stable selection | Blender prototype first; Rust only after differential fixtures |
| NURBS/Curves | blade spine/edge/profile curves, sweep/loft and deterministic tessellation | Rust curves subset; no general NURBS editor claim |
| Geometry Nodes | typed knife graphs for repetition, grooves, serrations and decorative patterning | Rust closed node subset; no arbitrary node parity |
| Retopology | silhouette/feature locks, quad-flow draft, correspondence and artist review | hybrid prototype plus Rust-owned Low truth |
| UV | seams, unwrap, pack, mirrored/stacked declarations, density/stretch diagnostics | Rust-owned receipt; audited library or Blender prototype may compute |
| Bake | normal/AO/curvature/thickness/ID with cage and miss/intersection diagnostics | hybrid worker initially; Rust validates every map and lineage |
| Animation | rigid handle/blade hierarchy, first-person idle/inspect/slash/stab clips and sockets | existing typed mechanical/FPS path, narrowed to knife |
| Plugins | fixed allowlisted internal providers with version/license/SBOM/fixtures/removal plan | never arbitrary public plugin execution |

## Blender prototype boundary

Each accepted prototype provider must pin Blender revision/version, plugin revision, license hashes,
Python dependency lock, headless command, resource limits and platform. Runtime writes one immutable input
package to CAS, launches an offline sibling process, receives only embedded result bytes and a content-free
receipt, then independently checks topology, finite values, units, transforms, UVs, tangent basis, image
channels, budgets and deterministic replay.

The provider lifecycle is:

`research-authorized → isolated-prototype → differential-shadow → accepted-internal → rust-replacement | removed`

Process isolation does not cancel GPL obligations. Distribution of Blender, modified Blender or GPL add-ons
requires a separate shipping/license decision; an internal prototype receipt does not authorize packaging.

## Public Tool strategy

The default Codex surface becomes a knife profile built around a small workflow vocabulary:

- session/capability/authorization and scene inspection;
- mesh transaction and modifier graph;
- High/Low/UV/Bake production;
- material production;
- FPS/animation and engine delivery;
- review, approval, version and export.

Legacy subject/version-specific tools remain callable only through an explicit compatibility profile until
their persisted records and replay paths are migrated. Hiding a tool is not deletion; physical deletion
requires the deletion manifest, recovery source and focused replay tests.

## Acceptance

Ten-day delivery requires one authorized knife and one original control knife on the same build and generic
workflow. Both must bind original/evaluated mesh, High/Low/UV/Cage/Bake/PBR, fixed AOV/FPS views, animation,
engine receipt, independent weapon artist review and explicit approval to the same candidate/export lineage.
Missing partner references, target engine profile or independent reviewer block commercial acceptance rather
than being inferred by Codex.
