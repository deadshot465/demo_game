use serde::{Deserialize, Serialize};

/// プレイヤーのJSONオブジェクト
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Player {
    #[serde(rename = "PlayerId")]
    pub player_id: String,
    #[serde(rename = "UserName")]
    pub user_name: String,
    #[serde(rename = "Email")]
    pub email: String,
    #[serde(rename = "Nickname")]
    pub nickname: String,
}
