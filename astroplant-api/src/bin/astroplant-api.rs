use axum::{
    extract::ws::WebSocketUpgrade,
    handler::Handler,
    http::Method,
    http::{header, Uri},
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Extension, Router,
};
use futures::StreamExt;
use tower_http::cors::CorsLayer;

use astroplant_api::{
    authorization,
    controllers::{
        kit, kit_configuration, kit_rpc, me, measurement, media, peripheral_definition, permission,
        quantity_type, user,
    },
    database, helpers, init_token_signer, models, mqtt,
    problem::{GenericProblem, Problem},
    response, DEFAULT_S3_ENDPOINT, DEFAULT_S3_REGION,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    astroplant_api::utils::tracing::init();

    init_token_signer();

    // FIXME: SQLx was added async streaming support, so currently we're using two SQL engines. It
    // would be a good idea to choose one.
    let pg = database::PgPool::new(std::time::Duration::from_secs(5));
    let sqlx_pg = database::new_sqlx_pool().await?;

    let object_store = astroplant_object::ObjectStore::s3(
        std::env::var("AWS_S3_REGION").unwrap_or(DEFAULT_S3_REGION.to_owned()),
        std::env::var("AWS_S3_ENDPOINT").unwrap_or(DEFAULT_S3_ENDPOINT.to_owned()),
    );

    // Start MQTT.
    let (mut raw_measurement_receiver, kits_rpc) = mqtt::run(pg.clone(), object_store.clone());

    // Start WebSockets.
    let (ws_publisher, ws_handler) = astroplant_websocket::create();

    tokio::spawn(async move {
        while let Some(raw_measurement) = raw_measurement_receiver.next().await {
            ws_publisher.publish_raw_measurement(raw_measurement).await;
        }
    });

    // TODO: implement rate limiting
    // let _rate_limit = rate_limit::leaky_bucket();

    let app = Router::new()
        .route("/ws", get(websocket_handler).layer(Extension(ws_handler)))
        .route(
            "/media/:media_id/content",
            get(media::download_media).layer(Extension(object_store)),
        )
        .route("/media/:media_id", delete(media::delete_media))
        .route("/kits", get(kit::kits))
        .route("/kits", post(kit::create_kit))
        .route("/kits/:kit_serial", get(kit::kit_by_serial))
        .route("/kits/:kit_serial", patch(kit::patch_kit))
        .route("/kits/:kit_serial", delete(kit::delete_kit))
        .route("/kits/:kit_serial/members", get(kit::get_members))
        .route("/kits/:kit_serial/members", post(kit::add_member))
        .route(
            "/kits/:kit_serial/member-suggestions",
            get(kit::get_member_suggestions),
        )
        .route("/kits/:kit_serial/password", post(kit::reset_password))
        .route(
            "/kits/:kit_serial/configurations",
            get(kit_configuration::configurations_by_kit_serial),
        )
        .route(
            "/kits/:kit_serial/configurations",
            post(kit_configuration::create_configuration),
        )
        .route(
            "/kits/:kit_serial/aggregate-measurements",
            get(measurement::kit_aggregate_measurements),
        )
        .route("/kits/:kit_serial/media", get(media::kit_media))
        .route("/kits/:kit_serial/archive", get(kit::archive))
        .route("/kits/:kit_serial/archive", post(kit::archive_authorize))
        .route(
            "/kit-memberships/:kit_membership_id",
            patch(kit::patch_kit_membership),
        )
        .route(
            "/kit-memberships/:kit_membership_id",
            delete(kit::delete_kit_membership),
        )
        .route(
            "/kit-configurations/:kit_configuration_id",
            patch(kit_configuration::patch_configuration),
        )
        .route(
            "/kit-configurations/:kit_configuration_id/peripherals",
            post(kit_configuration::add_peripheral_to_configuration),
        )
        .route(
            "/kit-configurations/:kit_configuration_id",
            delete(kit_configuration::delete_configuration),
        )
        .nest(
            "/kit-rpc",
            Router::new()
                .route("/:kit_serial/version", get(kit_rpc::version))
                .route("/:kit_serial/uptime", get(kit_rpc::uptime))
                .route(
                    "/:kit_serial/peripheral-command",
                    post(kit_rpc::peripheral_command),
                )
                .layer(Extension(kits_rpc)),
        )
        .route(
            "/peripherals/:peripheral_id",
            patch(kit_configuration::patch_peripheral),
        )
        .route(
            "/peripherals/:peripheral_id",
            delete(kit_configuration::delete_peripheral),
        )
        .route("/me", get(me::me))
        .route("/me/auth", post(me::authenticate_by_credentials))
        .route("/me/refresh", post(me::access_token_from_refresh_token))
        .route("/users/:username", get(user::user_by_username))
        .route("/users/:username", patch(user::patch_user))
        .route(
            "/users/:username/kit-memberships",
            get(user::list_kit_memberships),
        )
        .route("/users", post(user::create_user))
        .route(
            "/peripheral-definitions",
            get(peripheral_definition::peripheral_definitions),
        )
        .route("/permissions", get(permission::user_kit_permissions))
        .route(
            "/time",
            get(|| async { response::ResponseBuilder::ok().body(chrono::Utc::now().to_rfc3339()) }),
        )
        .route("/quantity-types", get(quantity_type::quantity_types))
        .layer(Extension(pg))
        .layer(Extension(sqlx_pg))
        .fallback(fallback.into_service())
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(
            // TODO: this layer might be better placed per-endpoint, to have accurate allowed methods
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
                .expose_headers([header::LINK, header::HeaderName::from_static("x-next")]),
        );

    // FIXME: make the listen address configurable
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));

    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

/// 404 handler
async fn fallback(_uri_: Uri) -> impl IntoResponse {
    Problem::Generic(GenericProblem::NotFound).into_response()
}

async fn websocket_handler(
    Extension(pg): Extension<database::PgPool>,
    Extension(ws_handle): Extension<astroplant_websocket::SocketHandler>,
    ws: WebSocketUpgrade,
    user_id: Option<models::UserId>,
) -> impl IntoResponse {
    ws.on_upgrade(move |ws| async move {
        ws_handle
            .handle(ws, move |kit_serial| {
                let pg = pg.clone();
                let user_id = user_id;
                async move {
                    helpers::fut_kit_permission_or_forbidden(
                        pg,
                        user_id,
                        kit_serial,
                        authorization::KitAction::SubscribeRealTimeMeasurements,
                    )
                    .await
                    .is_ok()
                }
            })
            .await;
    })
}
