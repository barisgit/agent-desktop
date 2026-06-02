use crate::{adapter::HitTestResult, node::Rect, refs_store::RefStore};
use serde_json::{Value, json};

/// Returns the snapshot ref id (`@e{n}`) for a hit-tested element when
/// the latest snapshot contains a matching entry.
///
/// Matches by `(pid, role, name, bounds_hash)` against the latest
/// snapshot's refmap. Any missing field or mismatch yields `None`; the
/// caller treats `None` as "no stable ref available, inline data only"
/// rather than an error.
pub fn ref_id_for_hit(hit: &HitTestResult, ref_store: &RefStore) -> Option<String> {
    let refmap = ref_store.load_latest().ok()?;
    let pid = hit.pid?;
    let hash = hit.bounds_hash;
    for (ref_id, entry) in refmap.iter() {
        if entry.pid == pid
            && entry.role == hit.role
            && entry.name == hit.name
            && entry.bounds_hash == hash
        {
            return Some(ref_id.clone());
        }
    }
    None
}

/// Serialise a `HitTestResult` plus optional ref id into the `element`
/// field used by coordinate-based command responses.
pub fn element_json(hit: &HitTestResult, ref_id: Option<&str>) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(id) = ref_id {
        obj.insert("ref_id".into(), Value::String(id.into()));
    }
    obj.insert("role".into(), Value::String(hit.role.clone()));
    if let Some(name) = &hit.name {
        obj.insert("name".into(), Value::String(name.clone()));
    }
    if let Some(b) = &hit.bounds {
        obj.insert("bounds".into(), rect_json(b));
    }
    if !hit.available_actions.is_empty() {
        obj.insert(
            "available_actions".into(),
            Value::Array(
                hit.available_actions
                    .iter()
                    .map(|a| Value::String(a.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(pid) = hit.pid {
        obj.insert("pid".into(), json!(pid));
    }
    Value::Object(obj)
}

fn rect_json(r: &Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "width": r.width, "height": r.height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::{HitTestResult, NativeHandle},
        node::Rect,
        refs::{RefEntry, RefMap},
        refs_store::RefStore,
        refs_test_support::HomeGuard,
    };

    fn hit(role: &str, name: Option<&str>, pid: i32, hash: Option<u64>) -> HitTestResult {
        HitTestResult {
            handle: NativeHandle::null(),
            role: role.into(),
            name: name.map(String::from),
            bounds: Some(Rect {
                x: 1035.0,
                y: 641.0,
                width: 60.0,
                height: 48.0,
            }),
            bounds_hash: hash,
            available_actions: vec!["AXPress".into()],
            pid: Some(pid),
        }
    }

    fn entry(role: &str, name: Option<&str>, pid: i32, hash: Option<u64>) -> RefEntry {
        RefEntry {
            pid,
            role: role.into(),
            name: name.map(String::from),
            value: None,
            description: None,
            states: vec![],
            bounds: None,
            bounds_hash: hash,
            available_actions: vec!["AXPress".into()],
            source_app: Some("Calculator".into()),
            source_window_id: None,
            source_window_title: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        }
    }

    fn save_refmap_with(entries: &[(&str, RefEntry)]) -> RefStore {
        let store = RefStore::new().unwrap();
        let mut map = RefMap::new();
        for (_, e) in entries {
            map.allocate(e.clone());
        }
        store.save_new_snapshot(&map).unwrap();
        store
    }

    #[test]
    fn ref_id_matches_normalized_role() {
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 89911, Some(0xABCD)))]);
        let h = hit("button", Some("5"), 89911, Some(0xABCD));
        let id = ref_id_for_hit(&h, &store);
        assert_eq!(id.as_deref(), Some("@e1"));
    }

    #[test]
    fn ref_id_none_when_hit_uses_raw_ax_role() {
        // Regression guard: refmap stores normalized roles. If the macOS
        // hit-test code ever stops normalizing ("AXButton" -> "button"),
        // this test catches it: raw AX role must not match.
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 1, Some(1)))]);
        let h = hit("AXButton", Some("5"), 1, Some(1));
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn ref_id_none_when_bounds_hash_differs() {
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 1, Some(1)))]);
        let h = hit("button", Some("5"), 1, Some(2));
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn ref_id_none_when_pid_differs() {
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 1, Some(1)))]);
        let h = hit("button", Some("5"), 2, Some(1));
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn ref_id_none_when_name_differs() {
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 1, Some(1)))]);
        let h = hit("button", Some("6"), 1, Some(1));
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn ref_id_none_when_hit_has_no_pid() {
        let _guard = HomeGuard::new();
        let store = save_refmap_with(&[("@e1", entry("button", Some("5"), 1, Some(1)))]);
        let mut h = hit("button", Some("5"), 1, Some(1));
        h.pid = None;
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn ref_id_none_when_no_snapshot_exists() {
        let _guard = HomeGuard::new();
        let store = RefStore::new().unwrap();
        let h = hit("button", Some("5"), 1, Some(1));
        assert_eq!(ref_id_for_hit(&h, &store), None);
    }

    #[test]
    fn element_json_full_shape() {
        let h = hit("button", Some("5"), 89911, Some(0xABCD));
        let v = element_json(&h, Some("@e28"));
        assert_eq!(v["ref_id"], "@e28");
        assert_eq!(v["role"], "button");
        assert_eq!(v["name"], "5");
        assert_eq!(v["pid"], 89911);
        assert_eq!(v["bounds"]["x"], 1035.0);
        assert_eq!(v["bounds"]["y"], 641.0);
        assert_eq!(v["bounds"]["width"], 60.0);
        assert_eq!(v["bounds"]["height"], 48.0);
        assert_eq!(v["available_actions"][0], "AXPress");
    }

    #[test]
    fn element_json_omits_ref_id_when_none() {
        let h = hit("button", Some("5"), 1, Some(1));
        let v = element_json(&h, None);
        assert!(
            v.get("ref_id").is_none(),
            "ref_id must be omitted, not null"
        );
    }

    #[test]
    fn element_json_omits_unset_fields() {
        let h = HitTestResult {
            handle: NativeHandle::null(),
            role: "group".into(),
            name: None,
            bounds: None,
            bounds_hash: None,
            available_actions: vec![],
            pid: None,
        };
        let v = element_json(&h, None);
        assert_eq!(v["role"], "group");
        assert!(v.get("name").is_none());
        assert!(v.get("bounds").is_none());
        assert!(v.get("available_actions").is_none());
        assert!(v.get("pid").is_none());
        assert!(v.get("ref_id").is_none());
    }
}
