use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub public_origin: String,
    pub snapshot_mode: bool,
    pub snapshot_token: String,
    pub session_secret: String,
    pub media_dir: PathBuf,
    pub app_env: String,
    pub asset_mode_vite: bool,
    pub vite_origin: String,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub secure_cookies: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        let development = app_env.eq_ignore_ascii_case("development");
        let snapshot_setting = std::env::var("SNAPSHOT_MODE").unwrap_or_default();
        let snapshot_mode = snapshot_setting == "on"
            || (!development && snapshot_setting != "off");
        let snapshot_token = std::env::var("SNAPSHOT_INTERNAL_TOKEN").unwrap_or_default();
        if snapshot_mode && snapshot_token.len() < 32 {
            panic!("SNAPSHOT_INTERNAL_TOKEN (32+ characters) is required when snapshot mode is on");
        }
        let public_origin = std::env::var("PUBLIC_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:5000".into())
            .trim_end_matches('/')
            .to_string();
        let session_secret = std::env::var("SESSION_SECRET").unwrap_or_else(|_| {
            if development {
                "development-session-secret-not-for-production".into()
            } else {
                panic!("SESSION_SECRET is required outside development");
            }
        });
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://hotel:hotel_local_only@localhost:5432/hotel".into()),
            public_origin: public_origin.clone(),
            snapshot_mode,
            snapshot_token,
            session_secret,
            media_dir: PathBuf::from(std::env::var("MEDIA_DIR").unwrap_or_else(|_| "./data/media".into())),
            app_env,
            asset_mode_vite: std::env::var("ASSET_MODE").unwrap_or_default() == "vite",
            vite_origin: std::env::var("VITE_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into()),
            admin_email: std::env::var("ADMIN_EMAIL").ok().filter(|value| !value.is_empty()),
            admin_password: std::env::var("ADMIN_PASSWORD").ok().filter(|value| !value.is_empty()),
            secure_cookies: public_origin.starts_with("https://"),
        }
    }

    pub fn for_tests(database_url: String) -> Self {
        Self {
            database_url,
            public_origin: "http://localhost:5000".into(),
            snapshot_mode: true,
            snapshot_token: "test-snapshot-token-with-32-characters-minimum".into(),
            session_secret: "test-session-secret-with-enough-length".into(),
            media_dir: PathBuf::from("./data/test-media"),
            app_env: "test".into(),
            asset_mode_vite: false,
            vite_origin: "http://localhost:5173".into(),
            admin_email: Some("admin@example.com".into()),
            admin_password: Some("ChangeMe123!".into()),
            secure_cookies: false,
        }
    }
}
