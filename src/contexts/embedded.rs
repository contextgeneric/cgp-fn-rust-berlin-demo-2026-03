use aws_sdk_s3::Client;
use cgp::prelude::*;
use sqlx::SqlitePool;

#[derive(HasField)]
pub struct EmbeddedApp {
    pub database: SqlitePool,
    pub storage_client: Client,
    pub profile_pictures_bucket_id: String,
}
