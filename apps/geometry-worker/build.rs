use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let vendor = manifest_dir.join("vendor/manifold");
    let include = vendor.join("include");
    let c_include = vendor.join("bindings/c/include");
    let c_bindings = vendor.join("bindings/c");
    let src = vendor.join("src");

    // Keep the third-party build closed and deterministic.  This is the
    // sequential, no-I/O Manifold slice accepted by the adoption receipt;
    // no CMake, package manager, network, or optional backend participates.
    let manifold_sources = [
        "boolean2.cpp",
        "boolean2_diagnostics.cpp",
        "boolean2_offset.cpp",
        "boolean2_predicates.cpp",
        "boolean2_sweep.cpp",
        "boolean3.cpp",
        "boolean_result.cpp",
        "constructors.cpp",
        "cross_section.cpp",
        "csg_tree.cpp",
        "edge_op.cpp",
        "execution_impl.cpp",
        "face_op.cpp",
        "impl.cpp",
        "manifold.cpp",
        "minkowski.cpp",
        "polygon.cpp",
        "properties.cpp",
        "quickhull.cpp",
        "sdf.cpp",
        "smoothing.cpp",
        "sort.cpp",
        "subdivision.cpp",
        "tree2d.cpp",
    ];
    let c_sources = [
        "box.cpp",
        "conv.cpp",
        "cross.cpp",
        "manifoldc.cpp",
        "rect.cpp",
    ];

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(&include)
        .include(&c_include)
        .include(&c_bindings)
        .include(&src)
        .define("MANIFOLD_NO_IOSTREAM", None)
        .define("MANIFOLD_NO_FILESYSTEM", None)
        .define("MANIFOLD_PAR", Some("-1"))
        .define("NDEBUG", None)
        .warnings(false);
    for file in manifold_sources {
        build.file(src.join(file));
    }
    for file in c_sources {
        build.file(c_bindings.join(file));
    }
    build.file(manifest_dir.join("src/manifold_bridge.cpp"));
    build.compile("forgecad_manifold");

    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src/manifold_bridge.cpp").display()
    );
    println!("cargo:rerun-if-changed={}", vendor.display());
}
