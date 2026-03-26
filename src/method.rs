use image::RgbImage;

use crate::contexts::app::App;
use crate::types::{User, UserId};

impl App {
    pub async fn get_user(&self, user_id: &UserId) -> anyhow::Result<User> {
        let user = sqlx::query_as(
            "SELECT name, email, profile_picture_object_id FROM users WHERE id = $1",
        )
        .bind(user_id.0 as i64)
        .fetch_one(&self.database)
        .await?;

        Ok(user)
    }

    pub async fn fetch_storage_object(&self, object_id: &str) -> anyhow::Result<Vec<u8>> {
        let output = self
            .storage_client
            .get_object()
            .bucket(&self.profile_pictures_bucket_id)
            .key(object_id)
            .send()
            .await?;

        let data = output.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }

    pub async fn get_user_profile_picture(
        &self,
        user_id: &UserId,
    ) -> anyhow::Result<Option<RgbImage>> {
        let user = self.get_user(user_id).await?;

        if let Some(object_id) = user.profile_picture_object_id {
            let data = self.fetch_storage_object(&object_id).await?;
            let image = image::load_from_memory(&data)?.to_rgb8();

            Ok(Some(image))
        } else {
            Ok(None)
        }
    }
}
