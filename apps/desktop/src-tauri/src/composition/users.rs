//! Local test identities. Names select a stable random actor; they never authenticate a person.
use crate::self_service::{error, Result};
use ai_session_contract::{TestUser, TestUserPage, UserContext};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
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
}
impl Users {
    pub fn open(root: &Path) -> Result<Self> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let path = root.join("users.json");
        let page = if path.exists() {
            let stat = fs::symlink_metadata(&path).map_err(|_| storage())?;
            if !stat.is_file()
                || stat.file_type().is_symlink()
                || stat.permissions().mode() & 0o077 != 0
            {
                return Err(storage());
            }
            let mut data = Vec::new();
            fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)
                .map_err(|_| storage())?
                .take(65537)
                .read_to_end(&mut data)
                .map_err(|_| storage())?;
            if data.len() > 65536 {
                return Err(storage());
            }
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
        let mut this = Self { path, page };
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
        this.persist(&this.page)?;
        Ok(this)
    }
    fn context(user: TestUser) -> Result<UserContext> {
        from_value(
            json!({"schemaVersion":5,"kind":"userContext","user":user,"generation":Uuid::new_v4().to_string()}),
        )
    }
    pub fn page(&self) -> TestUserPage {
        self.page.clone()
    }
    pub fn current(&self) -> Result<UserContext> {
        self.page
            .current
            .clone()
            .ok_or_else(|| error("user_required", "请先选择测试用户"))
    }
    pub fn require(&self, generation: &str) -> Result<UserContext> {
        let context = self.current()?;
        if context.generation.as_str() != generation {
            return Err(error("user_changed", "测试用户已切换，请刷新当前视图"));
        }
        Ok(context)
    }
    pub fn contains(&self, id: &str) -> bool {
        self.page
            .users
            .iter()
            .any(|user| user.user_id.as_str() == id)
    }
    pub fn select(&mut self, name: &str) -> Result<UserContext> {
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
        self.persist(&page)?;
        self.page = page;
        Ok(context)
    }
    fn persist(&self, page: &TestUserPage) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let temporary = self.path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)?;
            let data = serde_json::to_vec(page)?;
            if data.len() > 65536 {
                return Err("user registry capacity".into());
            }
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            fs::File::open(
                self.path
                    .parent()
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?,
            )?
            .sync_all()?;
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
        fs::create_dir(&root).unwrap();
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
