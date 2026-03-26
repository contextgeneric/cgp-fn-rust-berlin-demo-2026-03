use aws_sdk_s3::Client;
use cgp::prelude::*;
use image::RgbImage;
use sqlx::PgPool;

use crate::contexts::app::App;
use crate::contexts::minimal::MinimalApp;
use crate::contexts::smart::SmartApp;
use crate::types::{User, UserId};

pub async fn get_user(database: &PgPool, user_id: &UserId) -> anyhow::Result<User> {
    let user =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(database)
            .await?;

    Ok(user)
}

pub async fn fetch_storage_object(
    storage_client: &Client,
    profile_pictures_bucket_id: &str,
    object_id: &str,
) -> anyhow::Result<Vec<u8>> {
    let output = storage_client
        .get_object()
        .bucket(profile_pictures_bucket_id)
        .key(object_id)
        .send()
        .await?;

    let data = output.body.collect().await?.into_bytes().to_vec();
    Ok(data)
}

pub async fn get_user_profile_picture(
    database: &PgPool,
    storage_client: &Client,
    profile_pictures_bucket_id: &str,
    user_id: &UserId,
) -> anyhow::Result<Option<RgbImage>> {
    let user = get_user(database, user_id).await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let data =
            fetch_storage_object(storage_client, profile_pictures_bucket_id, &object_id).await?;

        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}