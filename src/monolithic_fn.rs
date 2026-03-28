use aws_sdk_s3::Client;
use image::RgbImage;
use sqlx::PgPool;

use crate::types::{User, UserId};

pub async fn get_user_profile_picture(
    database: &PgPool,
    storage_client: &Client,
    bucket_id: &str,
    user_id: &UserId,
) -> anyhow::Result<Option<RgbImage>> {
    let user: User =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(database)
            .await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let output = storage_client
            .get_object()
            .bucket(bucket_id)
            .key(object_id)
            .send()
            .await?;

        let data = output.body.collect().await?.into_bytes().to_vec();

        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}
