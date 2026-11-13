//! Supabase Auth (GoTrue) over REST: email and password, as the web and iOS apps sign in. The
//! session is shared with the worker threads, which refresh it before it expires and once more
//! after a 401, so a request never fails only because an hour passed. Refresh tokens rotate:
//! every refresh is saved back to the keychain.
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::net::http::{Method, NetError, Request, check};
use crate::store::secrets;

/// The Supabase project every Turbodoc client signs in to. The publishable key is public by
/// design; row-level security guards the data.
pub const SUPABASE_URL: &str = "https://itauvwgtmctneefilcff.supabase.co";
pub const SUPABASE_KEY: &str = "sb_publishable_yIVWfTAF5YQG-HmG2KBqqQ_G1apVgje";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub created_at: Option<String>,
    pub user_metadata: serde_json::Value,
}

impl User {
    /// A name to greet: the metadata's name, else the email's local part, capitalised.
    pub fn first_name(&self) -> String {
        for key in ["first_name", "full_name", "name"] {
            if let Some(n) = self.user_metadata.get(key).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                return n.split_whitespace().next().unwrap_or(n).to_owned();
            }
        }
        let local = self.email.as_deref().unwrap_or("").split('@').next().unwrap_or("");
        let word = local.split(['.', '_', '-', '+']).next().unwrap_or(local);
        let mut c = word.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: i64,
    pub expires_in: i64,
    pub user: User,
}

impl Session {
    fn normalised(mut self) -> Self {
        if self.expires_at == 0 {
            self.expires_at = now() + self.expires_in.max(60);
        }
        self
    }
    fn expiring(&self) -> bool { self.expires_at - now() < 90 }

    /// What the keychain keeps: the refresh token and whom it signs in. The access token stays
    /// out (the first request of a launch trades the refresh token for one), which keeps the
    /// item well inside Windows Credential Manager's limit: 2,560 bytes, 1,280 characters as
    /// the keyring crate stores them. A Supabase access token alone often passes 1,000.
    fn for_keychain(&self) -> Session {
        let names = ["first_name", "full_name", "name"];
        let meta: serde_json::Map<String, serde_json::Value> = names.iter().filter_map(|k| self.user.user_metadata.get(*k).map(|v| ((*k).to_owned(), v.clone()))).collect();
        Session {
            access_token: String::new(),
            refresh_token: self.refresh_token.clone(),
            expires_at: 0,
            expires_in: 0,
            user: User { id: self.user.id.clone(), email: self.user.email.clone(), created_at: self.user.created_at.clone(), user_metadata: serde_json::Value::Object(meta) },
        }
    }
}

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) }

fn auth_url(path: &str) -> String { format!("{SUPABASE_URL}/auth/v1/{path}") }

fn gotrue(method: Method, url: &str) -> Request<'_> { Request::new(method, url).header("apikey", SUPABASE_KEY) }

/// Signs in with email and password.
pub fn sign_in(email: &str, password: &str) -> Result<Session, NetError> {
    let url = auth_url("token?grant_type=password");
    let (status, body) = gotrue(Method::Post, &url).json(json!({ "email": email, "password": password })).send_raw()?;
    if status == 400 || status == 401 {
        // GoTrue says `invalid_grant` / "Invalid login credentials", or "Email not confirmed".
        let msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error_description").or(v.get("msg")).or(v.get("message")).and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "Invalid login credentials".into());
        return Err(NetError::Status(status, msg));
    }
    check(status, &body)?;
    serde_json::from_str::<Session>(&body).map(Session::normalised).map_err(|e| NetError::Decode(e.to_string()))
}

/// Creates an account. Supabase sends a confirmation email; the answer has a session only when
/// the project doesn't require one.
pub fn sign_up(email: &str, password: &str) -> Result<Option<Session>, NetError> {
    let url = auth_url("signup");
    let v: serde_json::Value = gotrue(Method::Post, &url).json(json!({ "email": email, "password": password })).send()?;
    if v.get("access_token").is_some() {
        return serde_json::from_value::<Session>(v).map(|s| Some(s.normalised())).map_err(|e| NetError::Decode(e.to_string()));
    }
    Ok(None)
}

/// Emails a password reset link (it opens the web app's update-password page).
pub fn recover(email: &str) -> Result<(), NetError> {
    let url = auth_url("recover");
    let _: serde_json::Value = gotrue(Method::Post, &url)
        .json(json!({ "email": email, "redirect_to": "https://turbodoc.ai/auth/update-password" }))
        .send()?;
    Ok(())
}

fn refresh(refresh_token: &str) -> Result<Session, NetError> {
    let url = auth_url("token?grant_type=refresh_token");
    let (status, body) = gotrue(Method::Post, &url).json(json!({ "refresh_token": refresh_token })).send_raw()?;
    if status == 400 || status == 401 || status == 403 {
        return Err(NetError::Unauthorized);
    }
    check(status, &body)?;
    serde_json::from_str::<Session>(&body).map(Session::normalised).map_err(|e| NetError::Decode(e.to_string()))
}

/// The signed-in session, shared by the UI and every worker.
///
/// The keychain is never touched on the UI thread: reading it can block for as long as macOS
/// shows its "allow access" prompt. [`Auth::pending`] starts without a session, a worker
/// [`Auth::restore_saved`]s it after the first frame, and requests made before that wait for it.
#[derive(Clone)]
pub struct Auth {
    session: Arc<Mutex<Option<Session>>>,
    refreshing: Arc<Mutex<()>>,
    /// Whether the saved session has been read (or there was none to read).
    ready: Arc<(Mutex<bool>, std::sync::Condvar)>,
}

impl Default for Auth {
    /// Ready and signed out: headless runs and tests.
    fn default() -> Auth { Auth { session: Default::default(), refreshing: Default::default(), ready: Arc::new((Mutex::new(true), std::sync::Condvar::new())) } }
}

impl Auth {
    /// Not yet read from the keychain; requests wait until [`Auth::restore_saved`] runs.
    pub fn pending() -> Auth {
        let auth = Auth::default();
        if let Ok(mut r) = auth.ready.0.lock() {
            *r = false;
        }
        auth
    }

    /// Reads the session saved by an earlier launch. Blocks on the keychain: call it on a worker.
    pub fn restore_saved(&self) -> Option<Session> {
        let saved = secrets::load().and_then(|j| serde_json::from_str::<Session>(&j).ok()).filter(|s| !s.refresh_token.is_empty());
        if saved.is_some() {
            self.set(saved.clone());
        }
        let (lock, cv) = &*self.ready;
        if let Ok(mut r) = lock.lock() {
            *r = true;
        }
        cv.notify_all();
        saved
    }

    /// Waits (on a worker) until the saved session has been read, for at most two minutes: as
    /// long as someone might take to answer the keychain's prompt.
    fn wait_ready(&self) {
        let (lock, cv) = &*self.ready;
        let Ok(guard) = lock.lock() else { return };
        let _ = cv.wait_timeout_while(guard, std::time::Duration::from_secs(120), |ready| !*ready);
    }

    pub fn session(&self) -> Option<Session> { self.session.lock().ok().and_then(|s| s.clone()) }
    pub fn user(&self) -> Option<User> { self.session().map(|s| s.user) }
    pub fn signed_in(&self) -> bool { self.session().is_some() }

    fn set(&self, s: Option<Session>) {
        if let Ok(mut slot) = self.session.lock() {
            *slot = s;
        }
    }

    /// Adopts a new session and saves it to the keychain (on a worker).
    pub fn adopt(&self, s: Session) {
        if let Ok(json) = serde_json::to_string(&s.for_keychain()) {
            crate::net::pool::spawn(move || {
                let _ = secrets::save(&json);
            });
        }
        self.set(Some(s));
    }

    /// Forgets the session here and in the keychain, and revokes it on the server (best effort,
    /// off the UI thread).
    pub fn sign_out(&self) {
        let token = self.session().map(|s| s.access_token).filter(|t| !t.is_empty());
        self.set(None);
        crate::net::pool::spawn(move || {
            secrets::clear();
            if let Some(token) = token {
                let url = auth_url("logout");
                let _ = gotrue(Method::Post, &url).bearer(&token).send_raw();
            }
        });
    }

    /// A token good for at least a minute and a half, refreshing first when needed.
    fn token(&self) -> Result<String, NetError> {
        self.wait_ready();
        let s = self.session().ok_or(NetError::Unauthorized)?;
        if s.expiring() { self.refresh_from(&s.access_token) } else { Ok(s.access_token) }
    }

    /// Refreshes unless another thread already replaced `stale`.
    fn refresh_from(&self, stale: &str) -> Result<String, NetError> {
        let _one = self.refreshing.lock().map_err(|_| NetError::Unauthorized)?;
        let s = self.session().ok_or(NetError::Unauthorized)?;
        if s.access_token != stale && !s.expiring() {
            return Ok(s.access_token);
        }
        match refresh(&s.refresh_token) {
            Ok(fresh) => {
                let token = fresh.access_token.clone();
                self.adopt(fresh);
                Ok(token)
            }
            Err(NetError::Unauthorized) => {
                // The refresh token was revoked or already used elsewhere: the session is over.
                self.set(None);
                secrets::clear();
                Err(NetError::Unauthorized)
            }
            Err(e) => Err(e),
        }
    }

    /// Runs `f` with a valid token, and once more with a refreshed one if the API says 401.
    pub fn with_token<T>(&self, f: impl Fn(&str) -> Result<T, NetError>) -> Result<T, NetError> {
        let token = self.token()?;
        match f(&token) {
            Err(NetError::Unauthorized) => {
                let token = self.refresh_from(&token)?;
                f(&token)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_metadata_or_email() {
        let mut u = User { email: Some("nico.botha@example.com".into()), ..Default::default() };
        assert_eq!(u.first_name(), "Nico");
        u.user_metadata = json!({ "full_name": "Ada Lovelace" });
        assert_eq!(u.first_name(), "Ada");
    }

    #[test]
    fn the_keychain_item_fits_windows_credential_manager() {
        let jwt = format!("eyJhbGciOiJIUzI1NiJ9.{}.sig", "x".repeat(1400));
        let s = Session {
            access_token: jwt,
            refresh_token: "v1.refresh-token-abcdefghijklmnop".into(),
            expires_at: now() + 3600,
            expires_in: 3600,
            user: User {
                id: "6a1c3f5e-1f2b-4c3d-9e8f-0123456789ab".into(),
                email: Some("someone.with.a.long.address@example.com".into()),
                created_at: Some("2026-01-01T00:00:00Z".into()),
                user_metadata: json!({ "full_name": "Ada Lovelace", "avatar_url": "x".repeat(400), "provider_id": "123" }),
            },
        };
        let stored = serde_json::to_string(&s.for_keychain()).unwrap();
        assert!(stored.chars().count() < 1280, "{} characters", stored.chars().count());
        let back: Session = serde_json::from_str(&stored).unwrap();
        assert_eq!(back.refresh_token, s.refresh_token);
        assert_eq!(back.user.first_name(), "Ada");
        assert!(back.expiring(), "a restored session refreshes before its first request");
    }

    #[test]
    fn requests_wait_for_the_saved_session_instead_of_failing() {
        let root = std::env::temp_dir().join(format!("td-auth-{}", std::process::id()));
        // SAFETY: set before any thread in this test reads them; no other test reads these.
        unsafe {
            std::env::set_var("TURBODOC_INSECURE_SESSION", "1");
            std::env::set_var("RUNE_DIRS_ROOT", &root);
        }
        let auth = Auth::pending();
        let session = Session { access_token: "a1".into(), refresh_token: "r1".into(), expires_in: 3600, ..Default::default() }.normalised();
        let file = rune::dirs::AppDirs::new(crate::APP_ID).unwrap().data().join("session.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, serde_json::to_string(&session).unwrap()).unwrap();

        let waiting = auth.clone();
        let started = std::time::Instant::now();
        let request = std::thread::spawn(move || waiting.with_token(|t| Ok(t.to_owned())));
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!request.is_finished(), "a request made before the restore waits for it");
        assert!(auth.restore_saved().is_some());
        assert_eq!(request.join().unwrap(), Ok("a1".to_owned()), "then runs with the restored token");
        assert!(started.elapsed() >= std::time::Duration::from_millis(150));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sessions_without_expires_at_get_one() {
        let s = Session { expires_in: 3600, ..Default::default() }.normalised();
        assert!(s.expires_at > now() + 3000 && !s.expiring());
    }
}
