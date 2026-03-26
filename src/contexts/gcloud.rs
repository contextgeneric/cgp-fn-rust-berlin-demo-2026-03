use cgp::prelude::*;
use google_cloud_storage::client::Storage;
use sqlx::PgPool;

#[derive(HasField)]
pub struct GCloudApp {
    pub database: PgPool,
    pub storage_client: Storage,
    pub profile_pictures_bucket_id: String,
}
