use cgp::prelude::*;
use sqlx::PgPool;

#[derive(HasField)]
pub struct MinimalApp {
    pub database: PgPool,
}
