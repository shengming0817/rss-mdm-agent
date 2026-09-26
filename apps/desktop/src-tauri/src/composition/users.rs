//! Local test identities. Names select a stable random actor; they never authenticate a person.
use crate::self_service::{error, Result};
use ai_session_contract::{TestUser, TestUserPage, UserContext};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub fn normalize(name: &str) -> Result<(String, String)> {
    let display: String = name.trim().nfc().collect();
    if !(1..=64).contains(&display.chars().count()) || display.chars().any(char::is_control) {
        return Err(error(
            "invalid_name",
            "测试用户名需为 1–64 个字符，且不能含控制字符",
        ));
    }
    let key = display
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect();
    Ok((display, key))
}
fn storage() -> crate::self_service::ServiceError {
    error("users_unavailable", "测试用户记录不可用")
}
fn from_value<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| storage())
}
pub struct Users {
    path: PathBuf,
    page: TestUserPage,
    active: Option<UserContext>,
}
impl Users {
    pub fn open(root: &Path) -> Result<Self> {
        let path = root.join("users.json");
        let page = if path.exists() {
            let data =
                native_process::private_storage::read(&path, 65536).map_err(|_| storage())?;
            let record = ai_session_contract::decode(
                &data,
                &ai_session_contract::Limits {
                    max_bytes: 65536,
                    max_text_bytes: 65536,
                    max_depth: 16,
                    max_nodes: 4096,
                },
            )
            .map_err(|_| storage())?;
            match record {
                ai_session_contract::WireRecord::TestUserPage(page) => page,
                _ => return Err(storage()),
            }
        } else {
            from_value(json!({"schemaVersion":5,"kind":"testUserPage","users":[]}))?
        };
        let mut this = Self {
            path,
            page,
            active: None,
        };
        let mut keys = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for user in &this.page.users {
            let (display, key) = normalize(&user.display_name)?;
            if display != *user.display_name
                || key != *user.name_key
                || !keys.insert(key)
                || !ids.insert(user.user_id.to_string())
            {
                return Err(storage());
            }
        }
        if let Some(current) = &this.page.current {
            let user = this
                .page
                .users
                .iter()
                .find(|user| user.user_id == current.user.user_id)
                .ok_or_else(storage)?
                .clone();
            this.page.current = Some(Self::context(user)?);
        }
        this.active = this.page.current.clone();
        this.persist(&this.page)?;
        Ok(this)
    }
    fn context(user: TestUser) -> Result<UserContext> {
        from_value(
            json!({"schemaVersion":5,"kind":"userContext","user":user,"generation":Uuid::new_v4().to_string()}),
        )
    }
    pub fn page(&self) -> TestUserPage {
        let mut page = self.page.clone();
        page.current = self.active.clone();
        page
    }
    pub fn current(&self) -> Result<UserContext> {
        self.active
            .clone()
            .ok_or_else(|| error("user_required", "请先选择测试用户"))
    }
    pub fn require(&self, generation: &str) -> Result<UserContext> {
        let context = self.current()?;
        if context
            .identity
            .as_ref()
            .and_then(|i| i.expires_at_ms)
            .is_some_and(|end| super::execution::now().map_or(true, |now| now >= end))
        {
            return Err(error("session_expired", "企业会话已过期，请重新登录"));
        }
        if context.generation.as_str() != generation {
            return Err(error("user_changed", "测试用户已切换，请刷新当前视图"));
        }
        Ok(context)
    }
    pub fn prepare(&self, name: &str) -> Result<TestUserPage> {
        let (display, key) = normalize(name)?;
        let mut page = self.page.clone();
        let user = match page.users.iter().find(|user| *user.name_key == key) {
            Some(user) => user.clone(),
            None => {
                if page.users.len() >= 128 {
                    return Err(error("limit", "测试用户数量已达上限"));
                }
                let user: TestUser = from_value(
                    json!({"schemaVersion":5,"kind":"testUser","userId":Uuid::new_v4().to_string(),"displayName":display,"nameKey":key}),
                )?;
                page.users.push(user.clone());
                user
            }
        };
        let context = Self::context(user)?;
        page.current = Some(context.clone());
        Ok(page)
    }
    pub fn commit(&mut self, page: TestUserPage) -> Result<UserContext> {
        let context = page.current.clone().ok_or_else(storage)?;
        self.persist(&page)?;
        self.page = page;
        self.active = Some(context.clone());
        Ok(context)
    }
    pub fn select(&mut self, name: &str) -> Result<UserContext> {
        let page = self.prepare(name)?;
        self.commit(page)
    }
    pub fn clear(&mut self) -> Result<()> {
        // Revoke memory first, including when persistence fails.
        self.active = None;
        self.page.current = None;
        self.persist(&self.page)
    }
    pub fn activate(&mut self, context: UserContext) -> Result<UserContext> {
        self.clear()?;
        self.active = Some(context.clone());
        Ok(context)
    }
    pub fn guest(&self) -> Result<UserContext> {
        let path = self.path.with_file_name("guest-id");
        let id = if path.exists() {
            let data = native_process::private_storage::read(&path, 64).map_err(|_| storage())?;
            Uuid::parse_str(std::str::from_utf8(&data).map_err(|_| storage())?)
                .map_err(|_| storage())?
        } else {
            let id = Uuid::new_v4();
            let mut file =
                native_process::private_storage::create_new(&path).map_err(|_| storage())?;
            file.write_all(id.to_string().as_bytes())
                .map_err(|_| storage())?;
            file.sync_all().map_err(|_| storage())?;
            id
        };
        from_value(json!({"schemaVersion":5,"kind":"userContext",
            "user":{"schemaVersion":5,"kind":"testUser","userId":id.to_string(),"displayName":"访客","nameKey":"guest"},
            "generation":Uuid::new_v4().to_string(),
            "identity":{"mode":"guest","authorityId":"desktop-guest","tenantId":"local-guest","principalId":id.to_string()}}))
    }
    pub fn select_guest(&mut self) -> Result<UserContext> {
        let context = self.guest()?;
        self.activate(context)
    }
    fn persist(&self, page: &TestUserPage) -> Result<()> {
        let temporary = self.path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = native_process::private_storage::create_new(&temporary)?;
            let data = serde_json::to_vec(page)?;
            if data.len() > 65536 {
                return Err("user registry capacity".into());
            }
            file.write_all(&data)?;
            file.sync_all()?;
            drop(file);
            native_process::private_storage::replace(&temporary, &self.path)?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(|_| storage())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guest_is_stable_separate_from_same_named_test_user_and_revokes_generation() {
        let root = std::env::temp_dir().join(format!("rss-guest-{}", Uuid::new_v4()));
        native_process::private_storage::directory(&root).unwrap();
        let mut users = Users::open(&root).unwrap();
        let test = users.select("访客").unwrap();
        let guest = users.select_guest().unwrap();
        assert_ne!(test.user.user_id, guest.user.user_id);
        assert!(users.require(test.generation.as_str()).is_err());
        users.clear().unwrap();
        assert!(users.require(guest.generation.as_str()).is_err());
        let next = users.select_guest().unwrap();
        assert_eq!(guest.user.user_id, next.user.user_id);
        assert_ne!(guest.generation, next.generation);
        assert_eq!(users.page().users.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn names_are_unicode_trimmed_nfc_and_ascii_insensitive() {
        assert_eq!(
            normalize("\u{2003}Alice\u{a0}").unwrap(),
            ("Alice".into(), "alice".into())
        );
        assert_eq!(normalize("e\u{301}").unwrap().0, "é");
        assert_ne!(normalize("É").unwrap().1, normalize("é").unwrap().1);
        for name in ["", "\t ", "a\0b", &"字".repeat(65)] {
            assert!(normalize(name).is_err());
        }
    }
    #[test]
    fn selection_restores_actor_but_rotates_generation_and_preserves_first_spelling() {
        let root = std::env::temp_dir().join(format!("rss-users-{}", Uuid::new_v4()));
        native_process::private_storage::directory(&root).unwrap();
        let mut users = Users::open(&root).unwrap();
        assert!(users.current().is_err());
        let a = users.select(" Alice ").unwrap();
        let b = users.select("Bob").unwrap();
        assert_ne!(a.user.user_id, b.user.user_id);
        assert!(users.require(a.generation.as_str()).is_err());
        let again = users.select("ALICE").unwrap();
        assert_eq!(a.user.user_id, again.user.user_id);
        assert_eq!(*again.user.display_name, "Alice");
        let restored = Users::open(&root).unwrap().current().unwrap();
        assert_eq!(restored.user.user_id, a.user.user_id);
        assert_ne!(restored.generation, again.generation);
        fs::remove_dir_all(root).unwrap();
    }
}
