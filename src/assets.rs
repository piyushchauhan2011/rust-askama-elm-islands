use std::fs;

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Assets {
    pub runtime_script: String,
    pub admin_script: String,
}

impl Assets {
    pub fn load(vite: bool, vite_origin: &str) -> Self {
        if vite {
            let origin = vite_origin.trim_end_matches('/');
            return Self {
                runtime_script: format!("{origin}/src/islands.js"),
                admin_script: format!("{origin}/src/admin.js"),
            };
        }
        let manifest = fs::read_to_string("static/assets/.vite/manifest.json")
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        if let Some(manifest) = manifest {
            let runtime = entry(&manifest, "src/islands.js");
            let admin = entry(&manifest, "src/admin.js");
            if let (Some(runtime), Some(admin)) = (runtime, admin) {
                return Self { runtime_script: runtime, admin_script: admin };
            }
        }
        Self {
            runtime_script: "/assets/islands.js".into(),
            admin_script: "/assets/admin.js".into(),
        }
    }
}

fn entry(manifest: &Value, key: &str) -> Option<String> {
    let file = manifest.get(key)?.get("file")?.as_str()?;
    Some(format!("/assets/{file}"))
}
