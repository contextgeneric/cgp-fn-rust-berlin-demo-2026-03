use aws_sdk_s3::Client;
use cgp::prelude::*;
use sqlx::PgPool;

#[derive(HasField)]
pub struct App {
    pub database: PgPool,
    pub storage_client: Client,
    pub bucket_id: String,
}
