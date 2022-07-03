use axum::Extension;
use serde::Deserialize;

use crate::database::PgPool;
use crate::problem::Problem;
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

#[derive(Deserialize)]
pub struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /quantity-types/?after=afterId` route.
pub async fn quantity_types(
    Extension(pg): Extension<PgPool>,
    crate::extract::Query(cursor): crate::extract::Query<CursorPage>,
) -> Result<Response, Problem> {
    let conn = pg.get().await?;
    let quantity_types = helpers::threadpool(move || {
        Ok::<_, Problem>(
            models::QuantityType::cursor_page(&conn, cursor.after, 100)?
                .into_iter()
                .map(views::QuantityType::from)
                .collect::<Vec<_>>(),
        )
    })
    .await?;
    let next_page_uri = quantity_types
        .last()
        .map(|last| format!("/quantity-types?after={}", last.id));
    let mut response_builder = ResponseBuilder::ok();
    if let Some(next_page_uri) = next_page_uri {
        response_builder = response_builder.next_page_uri(&next_page_uri);
    }
    Ok(response_builder.body(quantity_types))
}
