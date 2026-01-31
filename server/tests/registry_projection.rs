// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

use cxdb_server::projection::project_msgpack;
use cxdb_server::projection::{BytesRender, EnumRender, RenderOptions, TimeRender, U64Format};
use cxdb_server::registry::Registry;
use rmpv::Value;
use tempfile::tempdir;

fn default_options() -> RenderOptions {
    RenderOptions {
        bytes_render: BytesRender::Base64,
        u64_format: U64Format::String,
        enum_render: EnumRender::Label,
        time_render: TimeRender::Iso,
        include_unknown: true,
    }
}

#[test]
fn registry_ingest_and_project() {
    let dir = tempdir().expect("tempdir");
    let mut registry = Registry::open(dir.path()).expect("open registry");

    let bundle = r#"
    {
      "registry_version": 1,
      "bundle_id": "2025-12-19T00:00:00Z#test",
      "types": {
        "com.example.Message": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "role", "type": "u8", "enum": "com.example.Role" },
                "2": { "name": "text", "type": "string" }
              }
            }
          }
        }
      },
      "enums": {
        "com.example.Role": { "1": "system", "2": "user" }
      }
    }
    "#;

    registry
        .put_bundle("2025-12-19T00:00:00Z#test", bundle.as_bytes())
        .expect("put bundle");

    let desc = registry
        .get_type_version("com.example.Message", 1)
        .expect("descriptor");

    let mut map = Vec::new();
    map.push((Value::Integer(1.into()), Value::Integer(2.into())));
    map.push((Value::Integer(2.into()), Value::String("hello".into())));
    map.push((Value::Integer(9.into()), Value::Integer(42.into())));
    let value = Value::Map(map);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value).expect("encode msgpack");

    let options = RenderOptions {
        bytes_render: BytesRender::Base64,
        u64_format: U64Format::String,
        enum_render: EnumRender::Label,
        time_render: TimeRender::Iso,
        include_unknown: true,
    };

    let projection = project_msgpack(&buf, desc, &registry, &options).expect("project");
    let data = projection.data.as_object().expect("data object");
    assert_eq!(data.get("role").unwrap().as_str().unwrap(), "user");
    assert_eq!(data.get("text").unwrap().as_str().unwrap(), "hello");

    let unknown = projection.unknown.expect("unknown");
    let unknown_obj = unknown.as_object().expect("unknown object");
    assert!(unknown_obj.contains_key("9"));
}

#[test]
fn nested_type_references() {
    let dir = tempdir().expect("tempdir");
    let mut registry = Registry::open(dir.path()).expect("open registry");

    // Bundle with nested type references
    let bundle = r#"
    {
      "registry_version": 1,
      "bundle_id": "nested-test",
      "types": {
        "test:Item": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "item_type", "type": "string" },
                "2": { "name": "nested", "type": "ref", "ref": "test:Nested" },
                "3": { "name": "items", "type": "array", "items": { "type": "ref", "ref": "test:ArrayItem" } }
              }
            }
          }
        },
        "test:Nested": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "name", "type": "string" },
                "2": { "name": "value", "type": "int64" }
              }
            }
          }
        },
        "test:ArrayItem": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "id", "type": "string" },
                "2": { "name": "count", "type": "int32" }
              }
            }
          }
        }
      },
      "enums": {}
    }
    "#;

    registry
        .put_bundle("nested-test", bundle.as_bytes())
        .expect("put bundle");
    let desc = registry
        .get_type_version("test:Item", 1)
        .expect("descriptor");

    // Build msgpack with nested structures using numeric tags
    // Item { item_type: "foo", nested: { name: "bar", value: 42 }, items: [{ id: "x", count: 1 }] }
    let nested_map = vec![
        (Value::Integer(1.into()), Value::String("bar".into())),
        (Value::Integer(2.into()), Value::Integer(42.into())),
    ];
    let array_item = vec![
        (Value::Integer(1.into()), Value::String("x".into())),
        (Value::Integer(2.into()), Value::Integer(1.into())),
    ];
    let root_map = vec![
        (Value::Integer(1.into()), Value::String("foo".into())),
        (Value::Integer(2.into()), Value::Map(nested_map)),
        (
            Value::Integer(3.into()),
            Value::Array(vec![Value::Map(array_item)]),
        ),
    ];
    let value = Value::Map(root_map);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value).expect("encode msgpack");

    let projection = project_msgpack(&buf, desc, &registry, &default_options()).expect("project");
    let data = projection.data.as_object().expect("data object");

    // Check top-level field
    assert_eq!(data.get("item_type").unwrap().as_str().unwrap(), "foo");

    // Check nested type was projected correctly (not raw numeric keys)
    let nested = data
        .get("nested")
        .unwrap()
        .as_object()
        .expect("nested object");
    assert_eq!(nested.get("name").unwrap().as_str().unwrap(), "bar");
    assert_eq!(nested.get("value").unwrap().as_str().unwrap(), "42"); // u64 formatted as string

    // Check array items were projected correctly
    let items = data.get("items").unwrap().as_array().expect("items array");
    assert_eq!(items.len(), 1);
    let first_item = items[0].as_object().expect("first item");
    assert_eq!(first_item.get("id").unwrap().as_str().unwrap(), "x");
    assert_eq!(first_item.get("count").unwrap().as_i64().unwrap(), 1);
}

#[test]
fn bundle_with_renderer_parses() {
    let dir = tempdir().expect("tempdir");
    let mut registry = Registry::open(dir.path()).expect("open registry");

    // Bundle with renderer specification
    let bundle = r#"
    {
      "registry_version": 1,
      "bundle_id": "renderer-test",
      "types": {
        "test:Message": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "text", "type": "string" }
              },
              "renderer": {
                "esm_url": "builtin:MessageRenderer",
                "component": "MessageRendererWrapper",
                "integrity": "sha384-abc123"
              }
            }
          }
        }
      },
      "enums": {}
    }
    "#;

    registry
        .put_bundle("renderer-test", bundle.as_bytes())
        .expect("put bundle");

    // Verify the renderer was parsed and preserved
    let spec = registry
        .get_type_version("test:Message", 1)
        .expect("type version");

    let renderer = spec.renderer.as_ref().expect("renderer should exist");
    assert_eq!(renderer.esm_url, "builtin:MessageRenderer");
    assert_eq!(
        renderer.component.as_ref().unwrap(),
        "MessageRendererWrapper"
    );
    assert_eq!(renderer.integrity.as_ref().unwrap(), "sha384-abc123");
}

#[test]
fn bundle_without_renderer_backward_compat() {
    let dir = tempdir().expect("tempdir");
    let mut registry = Registry::open(dir.path()).expect("open registry");

    // Bundle without renderer (old format)
    let bundle = r#"
    {
      "registry_version": 1,
      "bundle_id": "no-renderer-test",
      "types": {
        "test:OldType": {
          "versions": {
            "1": {
              "fields": {
                "1": { "name": "value", "type": "int32" }
              }
            }
          }
        }
      },
      "enums": {}
    }
    "#;

    registry
        .put_bundle("no-renderer-test", bundle.as_bytes())
        .expect("put bundle");

    // Verify the type was ingested correctly without renderer
    let spec = registry
        .get_type_version("test:OldType", 1)
        .expect("type version");

    assert!(
        spec.renderer.is_none(),
        "renderer should be None for old bundles"
    );
    assert!(spec.fields.contains_key(&1));
}

#[test]
fn get_all_renderers() {
    let dir = tempdir().expect("tempdir");
    let mut registry = Registry::open(dir.path()).expect("open registry");

    // Bundle with multiple types, some with renderers
    let bundle = r#"
    {
      "registry_version": 1,
      "bundle_id": "multi-renderer-test",
      "types": {
        "test:TypeA": {
          "versions": {
            "1": {
              "fields": { "1": { "name": "a", "type": "string" } },
              "renderer": { "esm_url": "builtin:RendererA" }
            }
          }
        },
        "test:TypeB": {
          "versions": {
            "1": {
              "fields": { "1": { "name": "b", "type": "string" } }
            }
          }
        },
        "test:TypeC": {
          "versions": {
            "1": {
              "fields": { "1": { "name": "c1", "type": "string" } }
            },
            "2": {
              "fields": { "1": { "name": "c2", "type": "string" } },
              "renderer": { "esm_url": "builtin:RendererC", "component": "CWrapper" }
            }
          }
        }
      },
      "enums": {}
    }
    "#;

    registry
        .put_bundle("multi-renderer-test", bundle.as_bytes())
        .expect("put bundle");

    let renderers = registry.get_all_renderers();

    // TypeA has a renderer
    assert!(renderers.contains_key("test:TypeA"));
    assert_eq!(
        renderers.get("test:TypeA").unwrap().esm_url,
        "builtin:RendererA"
    );

    // TypeB has no renderer
    assert!(!renderers.contains_key("test:TypeB"));

    // TypeC uses latest version (v2) which has a renderer
    assert!(renderers.contains_key("test:TypeC"));
    let c_renderer = renderers.get("test:TypeC").unwrap();
    assert_eq!(c_renderer.esm_url, "builtin:RendererC");
    assert_eq!(c_renderer.component.as_ref().unwrap(), "CWrapper");
}
