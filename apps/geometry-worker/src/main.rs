//! One-shot, resource-bounded entry point for the product-owned geometry
//! worker. The Runtime is the only process allowed to select this executable;
//! the worker receives one typed request on stdin, produces one typed response
//! on stdout, and then exits.

use forgecad_worker_protocol::{
    build_cohort_sha256, validate_request, WorkerError, WorkerRequest, WorkerResponse,
    MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES, PRODUCTION_WEAPON_CAGE_OFFSET_ENTRY,
    PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION, PRODUCTION_WEAPON_GEOMETRIC_BAKE_ENTRY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION, PRODUCTION_WEAPON_HERO_MATERIAL_ENTRY,
    PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION, PRODUCTION_WEAPON_HERO_UV_LAYOUT_ENTRY,
    PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION,
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_ENTRY,
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION,
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_ENTRY,
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION, PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ENTRY,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION, PRODUCTION_WEAPON_LOW_RETOPOLOGY_ENTRY,
    PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION, PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_ENTRY,
    PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION, WORKER_PROTOCOL,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

const MEMORY_LIMIT_BYTES: libc::rlim_t = 512 * 1024 * 1024;
const CPU_LIMIT_SECONDS: libc::rlim_t = 10;
const SURFACE_BAKE_CPU_LIMIT_SECONDS: libc::rlim_t = 120;

/// Darwin does not expose an enforceable RLIMIT_AS on the supported host
/// profile. This allocator ceiling is a product-owned additional guard over
/// all Rust allocations made by the closed Worker code. It is deliberately not
/// described as an OS total-RSS limit: native/mmap accounting remains a
/// separately recorded gate.
struct BoundedAllocator;

static LIVE_ALLOCATION_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL_ALLOCATOR: BoundedAllocator = BoundedAllocator;

unsafe impl GlobalAlloc for BoundedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !reserve_allocation(layout.size()) {
            return std::ptr::null_mut();
        }
        let allocation = System.alloc(layout);
        if allocation.is_null() {
            release_allocation(layout.size());
        }
        allocation
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !reserve_allocation(layout.size()) {
            return std::ptr::null_mut();
        }
        let allocation = System.alloc_zeroed(layout);
        if allocation.is_null() {
            release_allocation(layout.size());
        }
        allocation
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
        release_allocation(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        if new_size > old_size && !reserve_allocation(new_size - old_size) {
            return std::ptr::null_mut();
        }
        let allocation = System.realloc(pointer, layout, new_size);
        if allocation.is_null() && new_size > old_size {
            release_allocation(new_size - old_size);
        } else if !allocation.is_null() && old_size > new_size {
            release_allocation(old_size - new_size);
        }
        allocation
    }
}

fn reserve_allocation(size: usize) -> bool {
    let maximum = MEMORY_LIMIT_BYTES as usize;
    let mut current = LIVE_ALLOCATION_BYTES.load(Ordering::Relaxed);
    loop {
        let Some(next) = current.checked_add(size) else {
            return false;
        };
        if next > maximum {
            return false;
        }
        match LIVE_ALLOCATION_BYTES.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

fn release_allocation(size: usize) {
    let mut current = LIVE_ALLOCATION_BYTES.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_sub(size);
        match LIVE_ALLOCATION_BYTES.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--build-identity"] {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-geometry-worker",
                "build_cohort_sha256": build_cohort_sha256()
            })
        );
        return;
    }
    if args == ["--isolated-once"] {
        std::process::exit(run_isolated_once(CPU_LIMIT_SECONDS));
    }
    if args == ["--isolated-once-2k"] {
        std::process::exit(run_isolated_once(SURFACE_BAKE_CPU_LIMIT_SECONDS));
    }
    if args == [PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_LOW_RETOPOLOGY_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_CAGE_OFFSET_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_GEOMETRIC_BAKE_ENTRY] {
        std::process::exit(run_isolated_once_for(
            SURFACE_BAKE_CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_HERO_MATERIAL_ENTRY] {
        std::process::exit(run_isolated_once_for(
            SURFACE_BAKE_CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_HERO_UV_LAYOUT_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION),
        ));
    }
    if args == [PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_ENTRY] {
        std::process::exit(run_isolated_once_for(
            CPU_LIMIT_SECONDS,
            Some(PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION),
        ));
    }

    #[cfg(debug_assertions)]
    {
        if args == ["--isolated-test-limits"] {
            std::process::exit(test_limits());
        }
        if args == ["--isolated-test-inherited-soft-cpu"] {
            std::process::exit(test_inherited_soft_cpu_limit());
        }
        if args == ["--isolated-test-sleep"] {
            std::process::exit(test_sleep());
        }
        if args == ["--isolated-test-allocator-limit"] {
            std::process::exit(test_allocator_limit());
        }
        if args == ["--isolated-test-crash"] {
            std::process::exit(73);
        }
        if args.len() == 2 && args[0] == "--isolated-test-fd-probe" {
            std::process::exit(test_fd_probe(&args[1]));
        }
    }

    eprintln!("forgecad-geometry-worker: expected --isolated-once");
    std::process::exit(64);
}

fn run_isolated_once(cpu_limit_seconds: libc::rlim_t) -> i32 {
    run_isolated_once_for(cpu_limit_seconds, None)
}

fn run_isolated_once_for(cpu_limit_seconds: libc::rlim_t, expected_operation: Option<&str>) -> i32 {
    // Limits are installed before reading attacker-controlled request bytes.
    // CPU/core failures are fatal. Darwin does not expose a portable total
    // address-space limit, so its optional memory rlimits are reported by the
    // focused evidence gate instead of being misrepresented as an enforced
    // total-RSS budget.
    if let Err(message) = apply_worker_limits(cpu_limit_seconds) {
        emit_response(error_response("invalid-request", "WORKER_LIMITS", message));
        return 1;
    }

    let request_bytes = match read_bounded_stdin(MAX_WORKER_REQUEST_BYTES) {
        Ok(bytes) => bytes,
        Err(message) => {
            emit_response(error_response(
                "invalid-request",
                "WORKER_PROTOCOL",
                message,
            ));
            return 1;
        }
    };
    let request = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => request,
        Err(_) => {
            emit_response(error_response(
                "invalid-request",
                "WORKER_PROTOCOL",
                "worker request is not valid strict JSON",
            ));
            return 1;
        }
    };
    if let Err(message) = validate_request(&request) {
        emit_response(error_response(
            &request.request_id,
            "WORKER_PROTOCOL",
            message,
        ));
        return 1;
    }
    let protected_operation = matches!(
        request.operation.as_str(),
        PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION
            | PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION
            | PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION
            | PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION
            | PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION
            | PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION
            | PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION
            | PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION
    );
    if expected_operation.is_some_and(|operation| request.operation != operation)
        || (protected_operation && expected_operation.is_none())
    {
        emit_response(error_response(
            &request.request_id,
            "WORKER_PROTOCOL",
            if protected_operation {
                "protected operation requires its dedicated isolated entry point"
            } else {
                "worker operation is not valid for this isolated entry point"
            },
        ));
        return 1;
    }

    let response = match forgecad_geometry_worker::worker_result(
        &serde_json::to_value(&request).expect("strict request serializes"),
    ) {
        Ok(result) => WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request.request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => error_response(&request.request_id, "GEOMETRY_REJECTED", error.to_string()),
    };
    let succeeded = response.ok;
    if !emit_response(response) || !succeeded {
        1
    } else {
        0
    }
}

fn error_response(
    request_id: &str,
    code: impl Into<String>,
    message: impl Into<String>,
) -> WorkerResponse {
    WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id: if request_id.is_empty() {
            "invalid-request".to_owned()
        } else {
            request_id.to_owned()
        },
        build_cohort_sha256: build_cohort_sha256(),
        ok: false,
        result: None,
        error: Some(WorkerError {
            code: code.into(),
            message: message.into().chars().take(512).collect(),
        }),
    }
}

fn emit_response(response: WorkerResponse) -> bool {
    let bytes = match serde_json::to_vec(&response) {
        Ok(bytes) if bytes.len().saturating_add(1) <= MAX_WORKER_RESPONSE_BYTES => bytes,
        _ => match serde_json::to_vec(&error_response(
            &response.request_id,
            "WORKER_PROTOCOL",
            "worker response exceeded the bounded protocol limit",
        )) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        },
    };
    let mut stdout = io::BufWriter::new(io::stdout());
    stdout.write_all(&bytes).is_ok() && stdout.write_all(b"\n").is_ok() && stdout.flush().is_ok()
}

fn read_bounded_stdin(limit: usize) -> Result<Vec<u8>, String> {
    let mut input = io::stdin().lock();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|_| "worker request could not be read".to_owned())?;
        if count == 0 {
            break;
        }
        if bytes.len().saturating_add(count) > limit {
            return Err("worker request exceeded the bounded protocol limit".to_owned());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.is_empty() {
        return Err("worker request is empty".to_owned());
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // fields are emitted only by debug-only limit evidence probes.
struct WorkerLimitState {
    address_space_rlimit_applied: bool,
    data_rlimit_applied: bool,
}

fn apply_worker_limits(cpu_limit_seconds: libc::rlim_t) -> Result<WorkerLimitState, String> {
    #[cfg(target_os = "macos")]
    unsafe {
        // Darwin reports `EINVAL` for RLIMIT_AS on this supported local
        // profile. Attempt the optional address/data limits before any request
        // byte is read, but do not treat either as a portable total-memory
        // gate. Runtime evidence must keep total-memory enforcement explicitly
        // NOT_RUN until a Darwin-wide equivalent is independently proven.
        let address_space_rlimit_applied = optional_memory_limit(libc::RLIMIT_AS)?;
        let data_rlimit_applied = optional_memory_limit(libc::RLIMIT_DATA)?;
        set_limit(libc::RLIMIT_CPU, cpu_limit_seconds).map_err(|error| error.to_string())?;
        set_limit(libc::RLIMIT_CORE, 0).map_err(|error| error.to_string())?;
        Ok(WorkerLimitState {
            address_space_rlimit_applied,
            data_rlimit_applied,
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("isolated geometry worker is supported only on macOS".to_owned())
    }
}

#[cfg(target_os = "macos")]
unsafe fn optional_memory_limit(resource: libc::c_int) -> Result<bool, String> {
    match set_limit(resource, MEMORY_LIMIT_BYTES) {
        Ok(()) => Ok(true),
        Err(LimitError {
            errno: libc::EINVAL,
            ..
        }) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(target_os = "macos")]
struct LimitError {
    resource: libc::c_int,
    errno: libc::c_int,
}

#[cfg(target_os = "macos")]
impl std::fmt::Display for LimitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "worker resource limit {} could not be applied (errno {})",
            self.resource, self.errno
        )
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_limit(resource: libc::c_int, value: libc::rlim_t) -> Result<(), LimitError> {
    let mut existing = std::mem::zeroed::<libc::rlimit>();
    if libc::getrlimit(resource, &mut existing) != 0 {
        return Err(LimitError {
            resource,
            errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        });
    }
    // Keep the existing hard limit and never raise an inherited soft limit.
    // Retaining `rlim_max` also avoids a Darwin EINVAL observed when an
    // unprivileged process tries to lower the hard ceiling.
    let limit = libc::rlimit {
        rlim_cur: value.min(existing.rlim_cur).min(existing.rlim_max),
        rlim_max: existing.rlim_max,
    };
    if libc::setrlimit(resource, &limit) != 0 {
        return Err(LimitError {
            resource,
            errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        });
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn test_limits() -> i32 {
    let limits = match apply_worker_limits(CPU_LIMIT_SECONDS) {
        Ok(limits) => limits,
        Err(error) => {
            println!("{}", serde_json::json!({"error":error}));
            return 1;
        }
    };
    #[cfg(target_os = "macos")]
    unsafe {
        let cpu = match read_required_limit(libc::RLIMIT_CPU) {
            Ok(limit) => limit,
            Err(_) => return 1,
        };
        let core = match read_required_limit(libc::RLIMIT_CORE) {
            Ok(limit) => limit,
            Err(_) => return 1,
        };
        // `getrlimit(RLIMIT_AS)` itself can be unsupported on Darwin. Keep
        // its diagnostic read optional rather than turning a correctly
        // recorded unsupported feature into a false test failure.
        let address_space = if limits.address_space_rlimit_applied {
            read_optional_limit(libc::RLIMIT_AS)
        } else {
            None
        };
        let data = if limits.data_rlimit_applied {
            read_optional_limit(libc::RLIMIT_DATA)
        } else {
            None
        };
        if (limits.address_space_rlimit_applied && address_space.is_none())
            || (limits.data_rlimit_applied && data.is_none())
        {
            return 1;
        }
        println!(
            "{}",
            serde_json::json!({
                "address_space_bytes": address_space,
                "data_bytes": data,
                "cpu_seconds": cpu.rlim_cur,
                "core_bytes": core.rlim_cur,
                "address_space_rlimit_applied":limits.address_space_rlimit_applied,
                "data_rlimit_applied":limits.data_rlimit_applied,
                "tracked_allocator_limit_bytes":MEMORY_LIMIT_BYTES
            })
        );
        return 0;
    }
    #[cfg(not(target_os = "macos"))]
    1
}

#[cfg(all(debug_assertions, target_os = "macos"))]
unsafe fn read_required_limit(resource: libc::c_int) -> Result<libc::rlimit, ()> {
    let mut limit = std::mem::zeroed::<libc::rlimit>();
    if libc::getrlimit(resource, &mut limit) == 0 {
        Ok(limit)
    } else {
        Err(())
    }
}

#[cfg(all(debug_assertions, target_os = "macos"))]
unsafe fn read_optional_limit(resource: libc::c_int) -> Option<libc::rlim_t> {
    let mut limit = std::mem::zeroed::<libc::rlimit>();
    (libc::getrlimit(resource, &mut limit) == 0).then_some(limit.rlim_cur)
}

#[cfg(debug_assertions)]
fn test_sleep() -> i32 {
    if apply_worker_limits(CPU_LIMIT_SECONDS).is_err() {
        return 1;
    }
    std::thread::sleep(std::time::Duration::from_secs(11));
    0
}

#[cfg(debug_assertions)]
fn test_inherited_soft_cpu_limit() -> i32 {
    #[cfg(target_os = "macos")]
    unsafe {
        // Model a parent that already imposed a stricter soft CPU ceiling.
        // `apply_worker_limits` must preserve it rather than widening the
        // child budget back to the normal ten seconds.
        if set_limit(libc::RLIMIT_CPU, 1).is_err()
            || apply_worker_limits(CPU_LIMIT_SECONDS).is_err()
        {
            return 1;
        }
        let Ok(cpu) = read_required_limit(libc::RLIMIT_CPU) else {
            return 1;
        };
        println!("{}", serde_json::json!({"cpu_seconds":cpu.rlim_cur}));
        if usize::try_from(cpu.rlim_cur)
            .ok()
            .is_some_and(|seconds| seconds > 0 && seconds <= 1)
        {
            0
        } else {
            1
        }
    }
    #[cfg(not(target_os = "macos"))]
    1
}

#[cfg(debug_assertions)]
fn test_allocator_limit() -> i32 {
    if apply_worker_limits(CPU_LIMIT_SECONDS).is_err() {
        return 1;
    }
    // Touch a modest real allocation first, then ask the product allocator
    // for a reservation that would exceed its accounting ceiling. This is
    // evidence for the closed Rust allocator guard only; it is not evidence
    // for an OS-enforced total-RSS cap.
    const STRESS_BYTES: usize = 32 * 1024 * 1024;
    let mut stress = Vec::<u8>::new();
    if stress.try_reserve_exact(STRESS_BYTES).is_err() {
        println!(
            "{}",
            serde_json::json!({
                "allocator_rejected_limit_reservation":false,
                "actual_allocation_bytes":0,
                "tracked_allocator_limit_bytes":MEMORY_LIMIT_BYTES,
                "error":"allocator could not make bounded stress allocation"
            })
        );
        return 1;
    }
    stress.resize(STRESS_BYTES, 0);
    let reserve = MEMORY_LIMIT_BYTES as usize;
    let mut bytes = Vec::<u8>::new();
    let rejected = bytes.try_reserve_exact(reserve).is_err();
    println!(
        "{}",
        serde_json::json!({
            "allocator_rejected_limit_reservation":rejected,
            "actual_allocation_bytes":STRESS_BYTES,
            "tracked_allocator_limit_bytes":MEMORY_LIMIT_BYTES
        })
    );
    if rejected {
        0
    } else {
        1
    }
}

#[cfg(debug_assertions)]
fn test_fd_probe(value: &str) -> i32 {
    if apply_worker_limits(CPU_LIMIT_SECONDS).is_err() {
        return 1;
    }
    let Ok(fd) = value.parse::<libc::c_int>() else {
        return 64;
    };
    let inherited = unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0;
    println!("{}", serde_json::json!({"fd_inherited":inherited}));
    if inherited {
        1
    } else {
        0
    }
}
