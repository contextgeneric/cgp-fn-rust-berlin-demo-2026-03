use aws_sdk_s3::Client;
use cgp::prelude::*;
use rig::agent::Agent;
use rig::providers::openai;
use sqlx::PgPool;

#[derive(HasField)]
pub struct SmartApp {
    pub database: PgPool,
    pub storage_client: Client,
    pub profile_pictures_bucket_id: String,
    pub open_ai_client: openai::Client,
    pub open_ai_agent: Agent<openai::CompletionModel>,
}
