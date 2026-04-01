use aws_sdk_s3::Client;
use cgp::prelude::*;
use image::RgbImage;
use sqlx::PgPool;

use crate::contexts::app::App;
use crate::contexts::minimal::MinimalApp;
use crate::contexts::smart::SmartApp;
use crate::types::{User, UserId};

#[async_trait]
pub trait GetUser {
    async fn get_user(&self, user_id: &UserId) -> anyhow::Result<User>;
}

impl<Context> GetUser for Context
where
    Self: HasField<Symbol!("database"), Value = PgPool>,
{
    async fn get_user(&self, user_id: &UserId) -> anyhow::Result<User> {
        let database = self.get_field(PhantomData::<Symbol!("database")>);

        let user = sqlx::query_as(
            "SELECT name, email, profile_picture_object_id FROM users WHERE id = $1",
        )
        .bind(user_id.0 as i64)
        .fetch_one(database)
        .await?;
        Ok(user)
    }
}

#[async_trait]
pub trait FetchStorageObject {
    async fn fetch_storage_object(&self, object_id: &str) -> anyhow::Result<Vec<u8>>;
}

impl<Context> FetchStorageObject for Context
where
    Self: HasField<Symbol!("storage_client"), Value = Client>
        + HasField<Symbol!("bucket_id"), Value = String>,
{
    async fn fetch_storage_object(&self, object_id: &str) -> anyhow::Result<Vec<u8>> {
        let storage_client = self.get_field(PhantomData::<Symbol!("storage_client")>);
        let bucket_id = self.get_field(PhantomData::<Symbol!("bucket_id")>).as_str();

        let output = storage_client
            .get_object()
            .bucket(bucket_id)
            .key(object_id)
            .send()
            .await?;
        let data = output.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }
}

#[async_trait]
pub trait GetUserProfilePicture {
    async fn get_user_profile_picture(&self, user_id: &UserId) -> anyhow::Result<Option<RgbImage>>;
}

impl<Context> GetUserProfilePicture for Context
where
    Self: GetUser + FetchStorageObject,
{
    async fn get_user_profile_picture(&self, user_id: &UserId) -> anyhow::Result<Option<RgbImage>> {
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

pub trait CheckGetUserProfilePicture: GetUserProfilePicture {}

impl CheckGetUserProfilePicture for App {}

pub trait CheckGetUser: GetUser {}

impl CheckGetUser for App {}
impl CheckGetUser for MinimalApp {}
impl CheckGetUser for SmartApp {}
