#[allow(async_fn_in_trait)]
pub trait DatabaseDriver {
    type Pool;
    type Error;
    type Config;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error>;
}

#[cfg(test)]
mod tests {
    // This crate defines only the `DatabaseDriver` trait with no associated
    // logic or default methods. There are no pure functions to unit-test.
    //
    // Each driver crate (driver-sqlite, driver-postgres, driver-mysql,
    // driver-clickhouse) implements this trait and should be tested
    // independently with integration tests backed by real database instances.

    #[test]
    fn database_driver_trait_has_no_pure_logic() {
        // The trait is a contract; its implementations are tested in their
        // respective crates.
    }
}
