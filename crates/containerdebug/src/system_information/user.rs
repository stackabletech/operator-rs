use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct User {
    pub name: Option<String>, // The name of the current user
    pub uid: u32,             // The user ID (UID)
    pub gid: u32,             // The group ID (GID)
}

impl User {
    pub fn collect_current() -> Self {
        let uid = users::get_current_uid();
        let user = Self {
            name: users::get_user_by_uid(uid).map(|user| user.name().to_string_lossy().to_string()),
            uid,
            gid: users::get_current_gid(),
        };
        tracing::info!(user.name, user.uid, user.gid, "current user");
        user
    }
}
