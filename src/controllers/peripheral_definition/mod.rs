use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up peripheral definitions router.");

    warp::path::end()
        .and(warp::get())
        .and(peripheral_definitions(pg.clone()))
        .boxed()
}

fn def_false() -> bool {
    false
}

#[derive(Copy, Clone, Deserialize)]
#[serde(rename_all="camelCase")]
struct QueryParams {
    after: Option<i32>,
    #[serde(default = "def_false")]
    with_expected_quantity_types: bool,
}

async fn get_definitions_and_expected_quantity_types(
    conn: PgPooled,
    query_params: QueryParams,
) -> Result<
    (
        Vec<models::PeripheralDefinition>,
        Option<Vec<Vec<models::PeripheralDefinitionExpectedQuantityType>>>,
    ),
    Rejection,
> {
    helpers::threadpool_diesel_ok(move || {
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
pub fn peripheral_definitions(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::query::query::<QueryParams>().and(pg).and_then(
        |query_params: QueryParams, conn: PgPooled| {
            async move {
                let (definitions, expected_quantity_types) =
                    get_definitions_and_expected_quantity_types(conn, query_params).await?;

                let next_page_uri = definitions.last().map(|last| {
                    format!(
                        "/peripheral-definitions?after={}&withExpectedQuantityTypes={}",
                        last.id, query_params.with_expected_quantity_types
                    )
                });

                let mut response_builder = ResponseBuilder::ok();

                if let Some(next_page_uri) = next_page_uri {
                    response_builder = response_builder.next_page_uri(next_page_uri);
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
                                        .map(|expected_quantity_type| {
                                            expected_quantity_type.quantity_type_id
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            })
                            .collect::<Vec<_>>();
                        Ok::<_, Rejection>(
                            response_builder.body(definitions_with_expected_quantity_types),
                        )
                    }
                    None => {
                        let definitions = definitions
                            .into_iter()
                            .map(|definition| views::PeripheralDefinition::from(definition))
                            .collect::<Vec<_>>();
                        Ok(response_builder.body(definitions))
                    }
                }
            }
        },
    )
}
