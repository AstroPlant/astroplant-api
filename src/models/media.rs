use crate::cursors;
use crate::schema::media;

use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};
use uuid::Uuid;
use validator::Validate;

#[rustfmt::skip]
use super::{
    Kit, KitId,
    KitConfiguration, KitConfigurationId,
    Peripheral, PeripheralId,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "media"]
pub struct MediaId(#[column_name = "id"] pub Uuid);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations, Validate)]
#[belongs_to(parent = "Kit", foreign_key = "kit_id")]
#[belongs_to(parent = "KitId", foreign_key = "kit_id")]
#[belongs_to(parent = "KitConfiguration", foreign_key = "kit_configuration_id")]
#[belongs_to(parent = "KitConfigurationId", foreign_key = "kit_configuration_id")]
#[belongs_to(parent = "Peripheral", foreign_key = "peripheral_id")]
#[belongs_to(parent = "PeripheralId", foreign_key = "peripheral_id")]
#[table_name = "media"]
pub struct Media {
    pub id: Uuid,
    pub peripheral_id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub datetime: DateTime<Utc>,
    pub name: String,
    pub r#type: String,
    pub metadata: serde_json::Value,
    pub size: i64,
}

impl Media {
    pub fn by_id(conn: &PgConnection, media_id: MediaId) -> QueryResult<Option<Self>> {
        media::table.find(&media_id.0).first(conn).optional()
    }

    pub fn page(
        conn: &PgConnection,
        kit_id: KitId,
        configuration_id: Option<i32>,
        peripheral_id: Option<i32>,
        cursor: Option<cursors::AggregateMeasurements>,
    ) -> QueryResult<Vec<Self>> {
        let mut query = media::table
            .filter(media::columns::kit_id.eq(kit_id.0))
            .into_boxed();

        if let Some(configuration_id) = configuration_id {
            query = query.filter(media::columns::kit_configuration_id.eq(configuration_id));
        }
        if let Some(peripheral_id) = peripheral_id {
            query = query.filter(media::columns::peripheral_id.eq(peripheral_id));
        }

        if let Some(cursors::AggregateMeasurements(datetime, id)) = cursor {
            query = query.filter(
                media::columns::datetime
                    .lt(datetime)
                    .or(media::columns::datetime
                        .eq(datetime)
                        .and(media::columns::id.lt(id))),
            )
        }
        query
            .order((media::dsl::datetime.desc(), media::dsl::id.desc()))
            .limit(cursors::Media::PER_PAGE as i64)
            .load(conn)
    }

    pub fn get_id(&self) -> MediaId {
        MediaId(self.id)
    }

    pub fn get_kit_id(&self) -> KitId {
        KitId(self.kit_id)
    }

    pub fn get_kit_configuration_id(&self) -> KitConfigurationId {
        KitConfigurationId(self.kit_configuration_id)
    }

    pub fn get_peripheral_id(&self) -> PeripheralId {
        PeripheralId(self.peripheral_id)
    }
}

#[derive(Clone, Debug, PartialEq, Insertable, Validate)]
#[table_name = "media"]
pub struct NewMedia {
    pub id: Uuid,
    pub peripheral_id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub datetime: DateTime<Utc>,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 255))]
    pub type_: String,
    pub metadata: serde_json::Value,
    pub size: i64,
}

impl NewMedia {
    pub fn new(
        id: Uuid,
        peripheral_id: PeripheralId,
        kit_id: KitId,
        kit_configuration_id: KitConfigurationId,
        datetime: DateTime<Utc>,
        name: String,
        type_: String,
        metadata: serde_json::Value,
        size: i64,
    ) -> Self {
        Self {
            id,
            kit_id: kit_id.0,
            peripheral_id: peripheral_id.0,
            kit_configuration_id: kit_configuration_id.0,
            datetime,
            name,
            type_,
            metadata,
            size,
        }
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<Media> {
        use crate::schema::media::dsl::*;

        diesel::insert_into(media)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<Media>(conn)
    }
}
