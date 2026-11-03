//! `test/manifest.test.js` for the native tab: the module the manifest names as the tab's entry is
//! there, inside the kit, a module Hangar will load, and the manifest is otherwise what it was.

use std::path::{Path, PathBuf};

use serde_json::Value;

const MANIFEST: &str = include_str!("../../hangar-kit.json");
const LIMIT: u64 = 64 * 1024 * 1024;

fn manifest() -> Value {
    serde_json::from_str(MANIFEST).expect("hangar-kit.json reads")
}

fn kit() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}

fn tab() -> Value {
    manifest()["tabs"][0].clone()
}

fn module() -> Vec<u8> {
    std::fs::read(kit().join(tab()["entry"].as_str().unwrap())).expect("the module is committed")
}

#[test]
fn the_tab_is_native_under_schema_2() {
    let manifest = manifest();
    assert_eq!(
        manifest["schema"], 2,
        "a tab whose entry is a module needs schema 2"
    );
    assert_eq!(manifest["hangar"], ">=2.0.0");
    let tab = tab();
    assert_eq!(tab["id"], "db");
    assert_eq!(tab["entry"], "tabs/db.wasm");
    assert!(tab.get("native").is_none(), "the entry is the module itself");
    assert!(
        !kit().join("tabs/db").exists(),
        "no HTML page is left beside the module"
    );
    assert_eq!(tab["singleton"], true);
}

#[test]
fn the_entry_names_a_wasm_file_inside_the_kit() {
    let native = tab()["entry"].as_str().unwrap().to_owned();
    assert!(native.ends_with(".wasm"));
    assert!(
        !native.starts_with('/') && !native.contains('\\') && !native.contains(':'),
        "{native}"
    );
    let parts: Vec<&str> = native.split('/').collect();
    assert!(
        parts.iter().all(|part| !part.is_empty()
            && *part != "."
            && *part != ".."
            && !part.starts_with('.')),
        "no hidden part, no way out: {native}"
    );
    let file = kit().join(&native);
    assert!(
        file.is_file(),
        "{native} is committed: run scripts/build-native.sh"
    );
    let real = file.canonicalize().unwrap();
    assert!(
        real.starts_with(kit()),
        "a link must not lead out of the kit"
    );
    let size = std::fs::metadata(&file).unwrap().len();
    assert!(
        size > 1024 && size <= LIMIT,
        "{size} bytes: a kit's file is 64 MB at most"
    );
    assert!(
        !std::fs::symlink_metadata(&file)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the module is the file itself"
    );
}

#[test]
fn the_permissions_are_the_tabs_exactly() {
    let mut declared: Vec<String> = tab()["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        [
            "command:list",
            "command:rows",
            "command:schema",
            "command:tables",
            "data.read",
            "data.write",
            "files.read",
            "panes.read",
            "panes.send"
        ]
    );
    assert_eq!(
        tab()["params"]["path"]["pattern"],
        manifest()["commands"]["rows"]["args"]["path"]["pattern"]
    );
}

#[test]
fn the_build_script_puts_the_module_where_the_manifest_says() {
    let script = std::fs::read_to_string(kit().join("scripts/build-native.sh")).unwrap();
    assert!(
        script.contains("wasm32-unknown-unknown"),
        "never wasm32-wasip1"
    );
    assert!(!script.contains("wasip1"));
    assert!(script.contains("kit_db_tab.wasm"));
    assert!(script.contains(tab()["entry"].as_str().unwrap()));
    let ignored = std::fs::read_to_string(kit().join(".gitignore")).unwrap();
    assert!(ignored.lines().any(|line| line.trim() == "native/target/"));
}

// ---------------------------------------------------------------- The module.

/// A LEB128 number and what is left.
fn leb(bytes: &[u8]) -> (u64, &[u8]) {
    let (mut value, mut shift) = (0u64, 0);
    for (at, byte) in bytes.iter().enumerate() {
        value |= u64::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return (value, &bytes[at + 1..]);
        }
    }
    panic!("a number runs off the end");
}

fn name(bytes: &[u8]) -> (String, &[u8]) {
    let (len, rest) = leb(bytes);
    let (text, rest) = rest.split_at(len as usize);
    (String::from_utf8(text.to_vec()).expect("a name"), rest)
}

fn limits(bytes: &[u8]) -> &[u8] {
    let flags = bytes[0];
    let (_, rest) = leb(&bytes[1..]);
    if flags & 1 == 1 { leb(rest).1 } else { rest }
}

/// The imports (`module.field`) and the exports of a module, read from its sections.
fn interface(wasm: &[u8]) -> (Vec<String>, Vec<String>) {
    assert_eq!(
        &wasm[..8],
        b"\0asm\x01\0\0\0",
        "a WebAssembly module, version 1"
    );
    let (mut imports, mut exports) = (Vec::new(), Vec::new());
    let mut rest = &wasm[8..];
    while !rest.is_empty() {
        let (id, after) = (rest[0], &rest[1..]);
        let (size, after) = leb(after);
        let (mut section, next) = after.split_at(size as usize);
        rest = next;
        match id {
            2 => {
                let (count, mut body) = leb(section);
                for _ in 0..count {
                    let (module, after) = name(body);
                    let (field, after) = name(after);
                    body = match after[0] {
                        0 => leb(&after[1..]).1,
                        1 => limits(&after[2..]),
                        2 => limits(&after[1..]),
                        3 => &after[3..],
                        other => panic!("an import of kind {other}"),
                    };
                    imports.push(format!("{module}.{field}"));
                }
            }
            7 => {
                let (count, body) = leb(section);
                section = body;
                for _ in 0..count {
                    let (export, after) = name(section);
                    section = leb(&after[1..]).1;
                    exports.push(export);
                }
            }
            _ => {}
        }
    }
    (imports, exports)
}

#[test]
fn the_module_is_a_tab_hangar_can_load() {
    let (imports, exports) = interface(&module());
    for import in &imports {
        assert!(
            ["hangar.log", "hangar.now"].contains(&import.as_str()),
            "{import}: a module imports the log and the clock and nothing else (no WASI, no JavaScript glue): \
             build it for wasm32-unknown-unknown"
        );
    }
    for needed in [
        "memory",
        "hangar_abi_version",
        "hangar_alloc",
        "hangar_init",
        "hangar_update",
        "hangar_view",
    ] {
        assert!(
            exports.iter().any(|export| export == needed),
            "{needed}: {exports:?}"
        );
    }
}

#[test]
fn nothing_personal_is_inside_the_module() {
    // The paths of the machine that built it (a home folder, a user's name) would be in a panic
    // message; the build script maps them away.
    let text = String::from_utf8_lossy(&module()).into_owned();
    // Built from parts: the Node tests refuse these words anywhere in the kit's files.
    for folder in ["home", "Users", "root"] {
        let forbidden = format!("/{folder}/");
        assert!(
            !text.contains(&forbidden),
            "{forbidden} is in the module: build it with scripts/build-native.sh"
        );
    }
    assert!(!text.contains(&format!("C:\\{}", "Users")));
}
