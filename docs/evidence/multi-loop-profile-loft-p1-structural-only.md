# multi-loop-profile-loft@1 independent Gate

当前证据等级：`STRUCTURAL_ONLY`。

运行：

```bash
python3 scripts/check_multi_loop_profile_loft_p1.py
```

当前结果：合同与正/负 fixture Gate `PASS`，18 个负向 case 通过。正向 fixture 使用冻结的
`stations[].components[]` 结构：`shell-core` 含 `outer + hole-a + hole-b`，`island-a`
是独立 component 且位于 `shell-core.hole-a` 的 void 内。checker 精确要求
`endpoint_caps=closed-solid-boolean`、`hole_policy=manifold-difference`，并拒绝跨 component
重复或跨站点不存在的 `hole_id`。

源码 cohort 已通过
`multi_loop_profile_loft_compiles_to_deterministic_strict_glb_with_lineage`：同一 GeometryProgram
重复编译的 GLB bytes 一致，strict readback 的 boundary/non-manifold edge 均为 0，
Part/Node lineage 存在。这是一条单 through-hole + island 的 focused Worker source Gate，
不是安装后 Runtime receipt，也不是完整双孔正向 fixture 的 genus 证据。

fixture 中的 `expected_genus(shell-core)=2`、`expected_genus(island-a)=0` 仍只是合同期望，
不能当作实际 genus、安装 cohort Runtime、真实参考相似度或完整候选证据。

接收真实 receipt 时，checker 还要求每个 through-hole 的 `manifold_error`、boundary/non-manifold/
winding 均为零，按稳定顺序的差分 genus 为 `hole-a=1`、`hole-b=2`，最终 component genus 为 2/0，所有 station hole containment，
稳定 `hole_id` 顺序和重复运行的 program/artifact hash 完全一致，并保持
`runtime_write_performed=false`、`quality_status=structural_only`。

曾观察到的 Runtime P0（把跨 component outer containment 一律判为 overlap）已在当前共享源码中加入
`outer ⊂ other.hole` 的例外；但本轮没有可用的当前源码编译器/Runtime 执行回执，因此仍需在主线
重建后用该正向 fixture 做 Runtime/Worker/GLB 执行 Gate，不能把源码修复写成运行时 PASS。
