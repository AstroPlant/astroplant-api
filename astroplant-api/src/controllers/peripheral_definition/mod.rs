use axum::Extension;
use serde::Deserialize;

use crate::database::PgPool;
use crate::problem::Problem;
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

fn def_false() -> bool {
    false
}

#[derive(Copy, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    after: Option<i32>,
    #[serde(default = "def_false")]
    with_expected_quantity_types: bool,
}

async fn get_definitions_and_expected_quantity_types(
    pg: PgPool,
    query_params: QueryParams,
) -> Result<
    (
        Vec<models::PeripheralDefinition>,
        Option<Vec<Vec<models::PeripheralDefinitionExpectedQuantityType>>>,
    ),
    Problem,
> {
    let conn = pg.get().await?;
    helpers::threadpool_result(move || {
        models::PeripheralDefinition::cursor_page(&conn, query_params.after, 100).and_then(
            |definitions| {
                if query_params.with_expected_quantity_types {
                    models::PeripheralDefinitionExpectedQuantityType::of_peripheral_definitions(
                        &conn,
                        &definitions,
                    )
                    .map(|quantity_types| (definitions, Some(quantity_types)))
                } else {
                    Ok((definitions, None))
                }
            },
        )
    })
    .await
}

/// Handles the `GET /peripheral-definitions/?after=afterId` route.
pub async fn peripheral_definitions(
    Extension(pg): Extension<PgPool>,
    crate::extract::Query(query_params): crate::extract::Query<QueryParams>,
) -> Result<Response, Problem> {
    let (definitions, expected_quantity_types) =
        get_definitions_and_expected_quantity_types(pg, query_params).await?;

    let next_page_uri = definitions.last().map(|last| {
        format!(
            "/peripheral-definitions?after={}&withExpectedQuantityTypes={}",
            last.id, query_params.with_expected_quantity_types
        )
    });

    let mut response_builder = ResponseBuilder::ok();

    if let Some(next_page_uri) = next_page_uri {
        response_builder = response_builder.next_page_uri(&next_page_uri);
    }

    match expected_quantity_types {
        Some(expected_quantity_types) => {
            let definitions_with_expected_quantity_types = definitions
                .into_iter()
                .zip(expected_quantity_types)
                .map(|(definition, expected_quantity_types)| {
                    let pd = views::PeripheralDefinition::from(definition);
                    pd.with_expected_quantity_types(
                        expected_quantity_types
                            .into_iter()
                            .map(|expected_quantity_type| expected_quantity_type.quantity_type_id)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            Ok(response_builder.body(definitions_with_expected_quantity_types))
        }
        None => {
            let definitions = definitions
                .into_iter()
                .map(views::PeripheralDefinition::from)
                .collect::<Vec<_>>();
            Ok(response_builder.body(definitions))
        }
    }
}
