// ForgeCAD-owned, bounded C ABI adapter for the accepted Manifold source
// slice.  This file is the only product code allowed to cross into the
// vendored Boolean implementation.  It passes typed MeshGL64 buffers in and
// copies a bounded, lineage-aware result out; it never exposes a Manifold
// object or accepts a path, script, URL, or user callback.

#include "manifold/manifoldc.h"

#include <chrono>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <new>
#include <vector>

extern "C" {

struct ForgeCADBooleanOutputV1 {
  int32_t status;
  int32_t manifold_error;
  size_t num_vertices;
  size_t num_triangles;
  double volume;
  double surface_area;
  int32_t genus;
  double* positions;
  uint64_t* indices;
  uint32_t* source_ids;
  uint64_t* face_ids;
};

enum ForgeCADBooleanStatusV1 {
  FORGECAD_BOOLEAN_OK = 0,
  FORGECAD_BOOLEAN_INVALID_ARGUMENT = 1,
  FORGECAD_BOOLEAN_MANIFOLD_ERROR = 2,
  FORGECAD_BOOLEAN_BUDGET_EXCEEDED = 3,
  FORGECAD_BOOLEAN_TIMEOUT = 4,
  FORGECAD_BOOLEAN_INTERNAL_ERROR = 5,
};

namespace {

using Clock = std::chrono::steady_clock;

struct ManifoldGuard {
  ManifoldManifold* value = nullptr;
  ~ManifoldGuard() {
    if (value != nullptr) {
      manifold_delete_manifold(value);
    }
  }
};

struct MeshGuard {
  ManifoldMeshGL64* value = nullptr;
  ~MeshGuard() {
    if (value != nullptr) {
      manifold_delete_meshgl64(value);
    }
  }
};

bool elapsed_over(const Clock::time_point& started, uint64_t limit_ms) {
  return std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now() - started)
             .count() > limit_ms;
}

bool valid_mesh(const double* positions, size_t vertices, const uint64_t* indices,
                size_t triangles, size_t max_vertices, size_t max_triangles) {
  if (positions == nullptr || indices == nullptr || vertices < 3 || triangles < 1 ||
      vertices > max_vertices || triangles > max_triangles || triangles > (SIZE_MAX / 3)) {
    return false;
  }
  for (size_t index = 0; index < vertices * 3; ++index) {
    if (!std::isfinite(positions[index])) {
      return false;
    }
  }
  for (size_t index = 0; index < triangles * 3; ++index) {
    if (indices[index] >= vertices) {
      return false;
    }
  }
  return true;
}

void clear_output(ForgeCADBooleanOutputV1* output) {
  if (output != nullptr) {
    std::memset(output, 0, sizeof(*output));
    output->status = FORGECAD_BOOLEAN_INTERNAL_ERROR;
  }
}

int fail(ForgeCADBooleanOutputV1* output, int status, int manifold_error) {
  clear_output(output);
  if (output != nullptr) {
    output->status = status;
    output->manifold_error = manifold_error;
  }
  return status;
}

int execute_boolean(
    int operation, const double* left_positions, size_t left_vertices,
    const uint64_t* left_indices, size_t left_triangles, const double* right_positions,
    size_t right_vertices, const uint64_t* right_indices, size_t right_triangles,
    size_t max_vertices, size_t max_triangles, uint64_t max_runtime_ms,
    ForgeCADBooleanOutputV1* output) {
  clear_output(output);
  if (output == nullptr || (operation < 0 || operation > 2) || max_vertices < 3 ||
      max_triangles < 1 || max_runtime_ms < 1 ||
      !valid_mesh(left_positions, left_vertices, left_indices, left_triangles,
                  max_vertices, max_triangles) ||
      !valid_mesh(right_positions, right_vertices, right_indices, right_triangles,
                  max_vertices, max_triangles)) {
    return fail(output, FORGECAD_BOOLEAN_INVALID_ARGUMENT, MANIFOLD_INVALID_CONSTRUCTION);
  }

  const auto started = Clock::now();
  std::vector<double> left_props(left_vertices * 3);
  std::vector<double> right_props(right_vertices * 3);
  std::memcpy(left_props.data(), left_positions, left_props.size() * sizeof(double));
  std::memcpy(right_props.data(), right_positions, right_props.size() * sizeof(double));
  std::vector<uint64_t> left_runs{0, static_cast<uint64_t>(left_triangles * 3)};
  std::vector<uint64_t> right_runs{0, static_cast<uint64_t>(right_triangles * 3)};
  std::vector<uint32_t> left_ids{1};
  std::vector<uint32_t> right_ids{2};
  ManifoldMeshGL64Options left_options{};
  left_options.run_indices = left_runs.data();
  left_options.run_indices_length = left_runs.size();
  left_options.run_original_ids = left_ids.data();
  left_options.run_original_ids_length = left_ids.size();
  ManifoldMeshGL64Options right_options{};
  right_options.run_indices = right_runs.data();
  right_options.run_indices_length = right_runs.size();
  right_options.run_original_ids = right_ids.data();
  right_options.run_original_ids_length = right_ids.size();

  MeshGuard left_mesh{manifold_alloc_meshgl64()};
  MeshGuard right_mesh{manifold_alloc_meshgl64()};
  if (left_mesh.value == nullptr || right_mesh.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  left_mesh.value = manifold_meshgl64_w_options(
      left_mesh.value, left_props.data(), left_vertices, 3,
      const_cast<uint64_t*>(left_indices), left_triangles, &left_options);
  right_mesh.value = manifold_meshgl64_w_options(
      right_mesh.value, right_props.data(), right_vertices, 3,
      const_cast<uint64_t*>(right_indices), right_triangles, &right_options);
  if (left_mesh.value == nullptr || right_mesh.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }

  ManifoldGuard left{nullptr};
  ManifoldGuard right{nullptr};
  left.value = manifold_of_meshgl64(manifold_alloc_manifold(), left_mesh.value);
  right.value = manifold_of_meshgl64(manifold_alloc_manifold(), right_mesh.value);
  if (left.value == nullptr || right.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  const ManifoldError left_error = manifold_status(left.value);
  const ManifoldError right_error = manifold_status(right.value);
  if (left_error != MANIFOLD_NO_ERROR || right_error != MANIFOLD_NO_ERROR) {
    const int error = left_error != MANIFOLD_NO_ERROR ? left_error : right_error;
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, error);
  }

  ManifoldGuard result{nullptr};
  result.value = manifold_boolean(
      manifold_alloc_manifold(), left.value, right.value,
      operation == 0 ? MANIFOLD_ADD
                     : operation == 1 ? MANIFOLD_SUBTRACT : MANIFOLD_INTERSECT);
  if (result.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  const ManifoldError result_error = manifold_status(result.value);
  if (result_error != MANIFOLD_NO_ERROR) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, result_error);
  }
  // Boolean intersections can leave machine-epsilon collinear edges in the
  // otherwise valid result.  Use Manifold's own tolerance-aware topology
  // simplifier before exporting source/face lineage; this removes only the
  // sliver topology that the same kernel identifies as redundant and keeps
  // the result inside the product-owned manifold contract.
  ManifoldGuard simplified{
      manifold_simplify(manifold_alloc_manifold(), result.value, 0.0)};
  if (simplified.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  const ManifoldError simplified_error = manifold_status(simplified.value);
  if (simplified_error != MANIFOLD_NO_ERROR) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, simplified_error);
  }
  manifold_delete_manifold(result.value);
  result.value = simplified.value;
  simplified.value = nullptr;
  if (elapsed_over(started, max_runtime_ms)) {
    return fail(output, FORGECAD_BOOLEAN_TIMEOUT, MANIFOLD_NO_ERROR);
  }
  if (manifold_is_empty(result.value)) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }

  const size_t vertices = manifold_num_vert(result.value);
  const size_t triangles = manifold_num_tri(result.value);
  if (vertices < 3 || triangles < 1 || vertices > max_vertices || triangles > max_triangles) {
    return fail(output, FORGECAD_BOOLEAN_BUDGET_EXCEEDED, MANIFOLD_RESULT_TOO_LARGE);
  }

  MeshGuard result_mesh{manifold_get_meshgl64(manifold_alloc_meshgl64(), result.value)};
  if (result_mesh.value == nullptr) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  if (manifold_meshgl64_num_vert(result_mesh.value) != vertices ||
      manifold_meshgl64_num_tri(result_mesh.value) != triangles ||
      manifold_meshgl64_num_prop(result_mesh.value) < 3 ||
      manifold_meshgl64_vert_properties_length(result_mesh.value) < vertices * 3 ||
      manifold_meshgl64_tri_length(result_mesh.value) != triangles * 3 ||
      manifold_meshgl64_face_id_length(result_mesh.value) != triangles) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_FACE_ID_WRONG_LENGTH);
  }

  const size_t run_index_length = manifold_meshgl64_run_index_length(result_mesh.value);
  const size_t run_id_length = manifold_meshgl64_run_original_id_length(result_mesh.value);
  if (run_index_length < 2 || run_id_length + 1 != run_index_length) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_RUN_INDEX_WRONG_LENGTH);
  }
  std::vector<double> props(vertices * manifold_meshgl64_num_prop(result_mesh.value));
  std::vector<uint64_t> tri_verts(triangles * 3);
  std::vector<uint64_t> run_indices(run_index_length);
  std::vector<uint32_t> run_ids(run_id_length);
  std::vector<uint64_t> face_ids(triangles);
  manifold_meshgl64_vert_properties(result_mesh.value ? props.data() : nullptr,
                                    result_mesh.value);
  manifold_meshgl64_tri_verts(tri_verts.data(), result_mesh.value);
  manifold_meshgl64_run_index(run_indices.data(), result_mesh.value);
  manifold_meshgl64_run_original_id(run_ids.data(), result_mesh.value);
  manifold_meshgl64_face_id(face_ids.data(), result_mesh.value);

  for (size_t index = 0; index < vertices; ++index) {
    if (!std::isfinite(props[index * manifold_meshgl64_num_prop(result_mesh.value) + 0]) ||
        !std::isfinite(props[index * manifold_meshgl64_num_prop(result_mesh.value) + 1]) ||
        !std::isfinite(props[index * manifold_meshgl64_num_prop(result_mesh.value) + 2])) {
      return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_NON_FINITE_VERTEX);
    }
  }
  for (uint64_t index : tri_verts) {
    if (index >= vertices) {
      return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR,
                  MANIFOLD_VERTEX_INDEX_OUT_OF_BOUNDS);
    }
  }
  for (size_t index = 0; index + 1 < run_indices.size(); ++index) {
    if (run_indices[index] > run_indices[index + 1] ||
        run_indices[index + 1] > triangles * 3) {
      return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_RUN_INDEX_WRONG_LENGTH);
    }
  }
  if (run_indices.back() != triangles * 3) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_RUN_INDEX_WRONG_LENGTH);
  }

  const size_t num_props = manifold_meshgl64_num_prop(result_mesh.value);
  const double volume = manifold_volume(result.value);
  const double surface_area = manifold_surface_area(result.value);
  if (!std::isfinite(volume) || !std::isfinite(surface_area) || volume <= 0.0) {
    return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }

  output->positions = static_cast<double*>(std::malloc(vertices * 3 * sizeof(double)));
  output->indices = static_cast<uint64_t*>(std::malloc(triangles * 3 * sizeof(uint64_t)));
  output->source_ids = static_cast<uint32_t*>(std::malloc(triangles * sizeof(uint32_t)));
  output->face_ids = static_cast<uint64_t*>(std::malloc(triangles * sizeof(uint64_t)));
  if (output->positions == nullptr || output->indices == nullptr ||
      output->source_ids == nullptr || output->face_ids == nullptr) {
    std::free(output->positions);
    std::free(output->indices);
    std::free(output->source_ids);
    std::free(output->face_ids);
    output->positions = nullptr;
    output->indices = nullptr;
    output->source_ids = nullptr;
    output->face_ids = nullptr;
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
  for (size_t index = 0; index < vertices; ++index) {
    output->positions[index * 3 + 0] = props[index * num_props + 0];
    output->positions[index * 3 + 1] = props[index * num_props + 1];
    output->positions[index * 3 + 2] = props[index * num_props + 2];
  }
  std::memcpy(output->indices, tri_verts.data(), triangles * 3 * sizeof(uint64_t));
  std::memcpy(output->face_ids, face_ids.data(), triangles * sizeof(uint64_t));
  std::memset(output->source_ids, 0xff, triangles * sizeof(uint32_t));
  for (size_t run = 0; run < run_id_length; ++run) {
    const uint32_t original_id = run_ids[run];
    if (original_id != 1 && original_id != 2) {
      std::free(output->positions);
      std::free(output->indices);
      std::free(output->source_ids);
      std::free(output->face_ids);
      output->positions = nullptr;
      output->indices = nullptr;
      output->source_ids = nullptr;
      output->face_ids = nullptr;
      return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_RUN_INDEX_WRONG_LENGTH);
    }
    const uint32_t source_id = original_id == 1 ? 0 : 1;
    const size_t first_triangle = static_cast<size_t>(run_indices[run] / 3);
    const size_t last_triangle = static_cast<size_t>(run_indices[run + 1] / 3);
    for (size_t triangle = first_triangle; triangle < last_triangle; ++triangle) {
      output->source_ids[triangle] = source_id;
    }
  }
  for (size_t triangle = 0; triangle < triangles; ++triangle) {
    if (output->source_ids[triangle] > 1) {
      std::free(output->positions);
      std::free(output->indices);
      std::free(output->source_ids);
      std::free(output->face_ids);
      output->positions = nullptr;
      output->indices = nullptr;
      output->source_ids = nullptr;
      output->face_ids = nullptr;
      return fail(output, FORGECAD_BOOLEAN_MANIFOLD_ERROR, MANIFOLD_RUN_INDEX_WRONG_LENGTH);
    }
  }

  output->status = FORGECAD_BOOLEAN_OK;
  output->manifold_error = MANIFOLD_NO_ERROR;
  output->num_vertices = vertices;
  output->num_triangles = triangles;
  output->volume = volume;
  output->surface_area = surface_area;
  output->genus = manifold_genus(result.value);
  if (elapsed_over(started, max_runtime_ms)) {
    // Do not return a late result as a successful bounded operation.  The
    // caller owns the deallocation contract even for this timeout branch.
    output->status = FORGECAD_BOOLEAN_TIMEOUT;
    output->manifold_error = MANIFOLD_NO_ERROR;
    return FORGECAD_BOOLEAN_TIMEOUT;
  }
  return FORGECAD_BOOLEAN_OK;
}

}  // namespace

int forgecad_manifold_boolean_v1(
    int operation, const double* left_positions, size_t left_vertices,
    const uint64_t* left_indices, size_t left_triangles, const double* right_positions,
    size_t right_vertices, const uint64_t* right_indices, size_t right_triangles,
    size_t max_vertices, size_t max_triangles, uint64_t max_runtime_ms,
    ForgeCADBooleanOutputV1* output) {
  try {
    return execute_boolean(operation, left_positions, left_vertices, left_indices,
                           left_triangles, right_positions, right_vertices, right_indices,
                           right_triangles, max_vertices, max_triangles, max_runtime_ms,
                           output);
  } catch (...) {
    return fail(output, FORGECAD_BOOLEAN_INTERNAL_ERROR, MANIFOLD_INVALID_CONSTRUCTION);
  }
}

void forgecad_manifold_boolean_free_v1(ForgeCADBooleanOutputV1* output) {
  if (output == nullptr) {
    return;
  }
  std::free(output->positions);
  std::free(output->indices);
  std::free(output->source_ids);
  std::free(output->face_ids);
  clear_output(output);
}

}  // extern "C"
