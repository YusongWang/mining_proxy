use diesel::{SqliteConnection, RunQueryDsl, OptionalExtension};

use super::models;
type DbError = Box<dyn std::error::Error + Send + Sync>;

/// Run query using Diesel to find user by uid and return it.
pub fn find_user_by_uid(
    conn: &SqliteConnection,
) -> Result<Option<models::Pool>, DbError> {
    use crate::web::schema::pools::dsl::*;

    let user = pools
        .first::<models::Pool>(conn)
        .optional()?;

    Ok(user)
}

pub fn get_all_pools(
    conn: &SqliteConnection,
) -> Result<Option<Vec<models::Pool>>, DbError> {
    use crate::web::schema::pools::dsl::*;

    let user = pools
        .load::<models::Pool>(conn)
        .optional()?;

    Ok(user)
}
