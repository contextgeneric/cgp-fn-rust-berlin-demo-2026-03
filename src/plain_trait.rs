use aws_sdk_s3::Client;
use image::RgbImage;
use rig::agent::Agent;
use rig::providers::openai;
use sqlx::PgPool;

use crate::contexts::app::App;
use crate::contexts::smart::SmartApp;
use crate::types::{User, UserId};

pub trait AppFields {
    fn database(&self) -> &PgPool;
    fn storage_client(&self) -> &Client;
    fn bucket_id(&self) -> &str;
}

pub trait SmartAppFields: AppFields {
    fn open_ai_agent(&self) -> &Agent<openai::CompletionModel>;

    fn open_ai_client(&self) -> &openai::Client;
}

pub async fn get_user<Context: AppFields>(
    context: &Context,
    user_id: &UserId,
) -> anyhow::Result<User> {
    let user =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(context.database())
            .await?;

    Ok(user)
}

pub async fn fetch_storage_object<Context: AppFields>(
    context: &Context,
    object_id: &str,
) -> anyhow::Result<Vec<u8>> {
    let output = context
        .storage_client()
        .get_object()
        .bucket(context.bucket_id())
        .key(object_id)
        .send()
        .await?;

    let data = output.body.collect().await?.into_bytes().to_vec();
    Ok(data)
}

pub async fn get_user_profile_picture<Context: AppFields>(
    context: &Context,
    user_id: &UserId,
) -> anyhow::Result<Option<RgbImage>> {
    let user = get_user(context, user_id).await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let data = fetch_storage_object(context, &object_id).await?;
        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}

impl AppFields for App {
    fn database(&self) -> &PgPool {
        &self.database
    }

    fn storage_client(&self) -> &Client {
        &self.storage_client
    }

    fn bucket_id(&self) -> &str {
        &self.bucket_id
    }
}

impl AppFields for SmartApp {
    fn database(&self) -> &PgPool {
        &self.database
    }

    fn storage_client(&self) -> &Client {
        &self.storage_client
    }

    fn bucket_id(&self) -> &str {
        &self.bucket_id
    }
}

impl SmartAppFields for SmartApp {
    fn open_ai_agent(&self) -> &Agent<openai::CompletionModel> {
        &self.open_ai_agent
    }

    fn open_ai_client(&self) -> &openai::Client {
        &self.open_ai_client
    }
}
