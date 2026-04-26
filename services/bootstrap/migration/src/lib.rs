pub use sea_orm_migration::prelude::*;

mod m20260424_000001_create_bootstrap_schema;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(
            m20260424_000001_create_bootstrap_schema::Migration,
        )]
    }
}
