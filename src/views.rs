use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EncodableKit {
    pub id: i32,
    pub serial: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub privacy_public_dashboard: bool,
    pub privacy_show_on_map: bool,
}
