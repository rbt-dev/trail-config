//! Benchmarks for `trail-config`.
//!
//! Everything here goes through the public API. Internals like `parse_path` are
//! `pub(super)` and unreachable from a bench target anyway, but measuring through
//! the surface users actually call is also the point: `str(path)` exercises path
//! parsing and tree navigation together, which is how the cost is really paid.

use std::collections::HashMap;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::Deserialize;
use tempfile::TempDir;
use trail_config::Config;

// ---------------------------------------------------------------- fixtures

/// A config nested `depth` levels deep, plus the path that reaches its leaf.
///
/// Returns e.g. (`"level0:\n  level1:\n    leaf: 42\n"`, `"level0/level1/leaf"`).
fn nested(depth: usize) -> (String, String) {
    let mut yaml = String::new();
    let mut path = String::new();

    for i in 0..depth {
        yaml.push_str(&"  ".repeat(i));
        yaml.push_str(&format!("level{i}:\n"));
        if i > 0 {
            path.push('/');
        }
        path.push_str(&format!("level{i}"));
    }
    yaml.push_str(&"  ".repeat(depth));
    yaml.push_str("leaf: 42\n");
    path.push_str("/leaf");

    (yaml, path)
}

/// A config with `sections` sibling sections, each a small map with a sequence.
fn wide(sections: usize) -> String {
    let mut yaml = String::new();
    for i in 0..sections {
        yaml.push_str(&format!(
            "section{i}:\n  \
               host: host{i}.example.com\n  \
               port: {}\n  \
               enabled: true\n  \
               tags:\n    - alpha\n    - beta\n",
            8000 + i
        ));
    }
    yaml
}

/// The same depth as `nested`, but every level's key contains the separator, so
/// each segment of the lookup path needs an escape sequence.
fn nested_escaped(depth: usize) -> (String, String) {
    let mut yaml = String::new();
    let mut path = String::new();

    for i in 0..depth {
        yaml.push_str(&"  ".repeat(i));
        yaml.push_str(&format!("\"lev/el{i}\":\n"));
        if i > 0 {
            path.push('/');
        }
        path.push_str(&format!("lev\\/el{i}"));
    }
    yaml.push_str(&"  ".repeat(depth));
    yaml.push_str("leaf: 42\n");
    path.push_str("/leaf");

    (yaml, path)
}

/// Deserialization target for `typed_access`. Most fields exist to give the
/// deserializer realistic work, not to be read back.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Section {
    host: String,
    port: u16,
    enabled: bool,
    tags: Vec<String>,
}

// ---------------------------------------------------------------- benchmarks

/// Path splitting plus tree navigation, by path depth.
///
/// This is the hot path for every accessor call, and the guard for items 6 and 17.
fn path_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_traversal");

    for depth in [1usize, 3, 10, 30] {
        let (yaml, path) = nested(depth);
        let config = Config::load_yaml(&yaml, "/").unwrap();

        group.bench_with_input(BenchmarkId::new("str", depth), &path, |b, path| {
            b.iter(|| black_box(config.str(black_box(path))));
        });
        group.bench_with_input(BenchmarkId::new("contains", depth), &path, |b, path| {
            b.iter(|| black_box(config.contains(black_box(path))));
        });
    }

    group.finish();
}

/// The same traversal where every segment carries an escape sequence — the branch
/// that cannot borrow if `parse_path` ever moves to `Cow` (item 17).
fn path_traversal_escaped(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_traversal_escaped");

    for depth in [1usize, 3, 10] {
        let (yaml, path) = nested_escaped(depth);
        let config = Config::load_yaml(&yaml, "/").unwrap();
        // Fail loudly here rather than benchmarking a lookup that misses
        assert_eq!(config.str(&path), "42", "escaped path {path:?} did not resolve");

        group.bench_with_input(BenchmarkId::new("str", depth), &path, |b, path| {
            b.iter(|| black_box(config.str(black_box(path))));
        });
    }

    group.finish();
}

/// Typed access on a large tree. `deserialize_strict` clones the entire document
/// and `get_as_strict` clones a subtree, per item 14 — the gap between the two
/// curves is that cost.
fn typed_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("typed_access");

    for sections in [1usize, 10, 100] {
        let config = Config::load_yaml(&wide(sections), "/").unwrap();

        group.bench_with_input(
            BenchmarkId::new("deserialize_strict_whole_tree", sections),
            &sections,
            |b, _| {
                b.iter(|| {
                    let parsed: HashMap<String, Section> =
                        config.deserialize_strict().unwrap();
                    black_box(parsed.len())
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_as_strict_one_subtree", sections),
            &sections,
            |b, _| {
                b.iter(|| {
                    let section: Section = config.get_as_strict("section0").unwrap();
                    black_box(section.port)
                });
            },
        );

        // Scalar read of the same field, as a floor for the two above
        group.bench_with_input(
            BenchmarkId::new("get_int_one_field", sections),
            &sections,
            |b, _| {
                b.iter(|| black_box(config.get_int("section0/port")));
            },
        );
    }

    group.finish();
}

/// Parsing a document from a string, by size. No file I/O.
fn loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("loading");

    for sections in [1usize, 10, 100] {
        let yaml = wide(sections);
        group.bench_with_input(
            BenchmarkId::new("load_yaml", sections),
            &yaml,
            |b, yaml| {
                b.iter(|| black_box(Config::load_yaml(black_box(yaml), "/").unwrap()));
            },
        );
    }

    group.finish();
}

/// Hot reload from disk, with and without an overlay. Includes file I/O, so the
/// absolute numbers are dominated by the OS — the useful signal is the delta
/// between the two variants and across sizes.
fn reload(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload");

    for sections in [1usize, 10, 100] {
        let dir = TempDir::new().unwrap();

        let base_path = dir.path().join("base.yaml");
        std::fs::write(&base_path, wide(sections)).unwrap();
        let base_path = base_path.to_str().unwrap();

        let overlay_path = dir.path().join("overlay.yaml");
        std::fs::write(&overlay_path, "section0:\n  port: 9999\n").unwrap();
        let overlay_path = overlay_path.to_str().unwrap();

        let mut plain = Config::load_required(base_path, "/", None).unwrap();
        group.bench_with_input(
            BenchmarkId::new("base_only", sections),
            &sections,
            |b, _| {
                b.iter(|| {
                    plain.reload().unwrap();
                    black_box(plain.get_int("section0/port"))
                });
            },
        );

        let mut overlaid = Config::load_required(base_path, "/", None)
            .unwrap()
            .merge_required(overlay_path, None)
            .unwrap();
        group.bench_with_input(
            BenchmarkId::new("base_plus_overlay", sections),
            &sections,
            |b, _| {
                b.iter(|| {
                    overlaid.reload().unwrap();
                    black_box(overlaid.get_int("section0/port"))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    path_traversal,
    path_traversal_escaped,
    typed_access,
    loading,
    reload
);
criterion_main!(benches);
