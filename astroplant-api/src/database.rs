use deadpool_diesel::postgres::{Manager, Pool};
use deadpool_diesel::Runtime;
use diesel::pg::PgConnection;
use diesel::ConnectionResult;

use crate::diesel::Connection as DieselConnection;
use crate::problem::{AppResult, Problem};

#[derive(Clone)]
pub struct PgPool(deadpool_diesel::postgres::Pool);

pub struct Connection(deadpool_diesel::postgres::Connection);

impl PgPool {
    pub fn new(connection_timeout: std::time::Duration) -> Self {
        let manager = Manager::new(&database_url(), Runtime::Tokio1);
        let pool = Pool::builder(manager)
            .runtime(Runtime::Tokio1)
            .max_size(8)
            .create_timeout(Some(connection_timeout))
            .build()
            .unwrap();
        Self(pool)
    }

    pub async fn get(self) -> AppResult<Connection> {
        self.0.get().await.map_err(Problem::from).map(Connection)
    }
}

impl Connection {
    /// Interact with the underlying connection on a separate thread.
    pub async fn interact<F, R>(&self, f: F) -> Result<R, deadpool_diesel::InteractError>
    where
        F: FnOnce(&mut diesel::pg::PgConnection) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.0.interact(f).await
    }

    /// Interact with the underlying connection on a separate thread.
    ///
    /// This helper method converts pool errors into `Problem`, taking a callback returning an
    /// `AppResult` (i.e., `Result<_, Problem>`), and flattening both errors into a single
    /// `AppResult<R>`.
    pub async fn interact_flatten_err<F, R, E>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&mut diesel::pg::PgConnection) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
        Problem: From<E>,
    {
        Ok(self.0.interact(f).await??)
    }
}

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or(crate::DEFAULT_DATABASE_URL.to_owned())
}

pub async fn new_sqlx_pool() -> Result<sqlx::postgres::PgPool, sqlx::Error> {
    // FIXME: SQLx was added for async streaming support, so currently we're using two SQL engines.
    // It would be a good idea to choose one.
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url())
        .await
}

pub fn oneoff_connection() -> ConnectionResult<PgConnection> {
    diesel::PgConnection::establish(&database_url())
}
