#[allow(async_fn_in_trait)]
pub trait DatabaseDriver {
    type Pool;
    type Error;
    type Config;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error>;
}
