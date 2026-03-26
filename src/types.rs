pub struct UserId(pub u64);

#[derive(sqlx::FromRow)]
pub struct User {
    pub name: String,
    pub email: String,
    pub profile_picture_object_id: Option<String>,
}
