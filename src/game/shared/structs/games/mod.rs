use crate::protos::grpc_service::game_state::{
    EntityState, Player, PlayerState, RoomState, WorldMatrix,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorldMatrixUdp {
    pub position: Vec<f32>,
    pub scale: Vec<f32>,
    pub rotation: Vec<f32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EntityStateUdp {
    pub current_hp: i32,
    pub max_hp: i32,
    pub current_sp: i32,
    pub max_sp: i32,
    pub is_alive: bool,
    pub world_matrix: WorldMatrixUdp,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlayerStateUdp {
    pub is_in_game: bool,
    pub room_id: String,
    pub is_owner: bool,
    pub state: EntityStateUdp,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlayerUdp {
    pub player_id: String,
    pub user_name: String,
    pub nickname: String,
    pub password: String,
    pub join_date: String,
    pub last_login: String,
    pub win_count: i32,
    pub lose_count: i32,
    pub credits: i32,
    pub email: String,
    pub state: PlayerStateUdp,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoomStateUdp {
    pub room_id: String,
    pub room_name: String,
    pub current_players: i32,
    pub max_players: i32,
    pub started: bool,
    pub message: String,
    pub players: Vec<PlayerUdp>,
}

impl Default for WorldMatrixUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldMatrixUdp {
    pub fn new() -> Self {
        WorldMatrixUdp {
            position: vec![],
            scale: vec![],
            rotation: vec![],
        }
    }
}

impl From<WorldMatrix> for WorldMatrixUdp {
    fn from(m: WorldMatrix) -> Self {
        WorldMatrixUdp {
            position: m.position.clone(),
            scale: m.scale.clone(),
            rotation: m.rotation.clone(),
        }
    }
}

impl Default for EntityStateUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityStateUdp {
    pub fn new() -> Self {
        EntityStateUdp {
            current_hp: 0,
            max_hp: 0,
            current_sp: 0,
            max_sp: 0,
            is_alive: false,
            world_matrix: WorldMatrixUdp::default(),
        }
    }
}

impl From<EntityState> for EntityStateUdp {
    fn from(state: EntityState) -> Self {
        EntityStateUdp {
            current_hp: state.current_hp,
            max_hp: state.max_hp,
            current_sp: state.current_sp,
            max_sp: state.max_sp,
            is_alive: state.is_alive,
            world_matrix: WorldMatrixUdp::from(
                state.world_matrix.expect("Failed to get world matrix."),
            ),
        }
    }
}

impl Default for PlayerStateUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerStateUdp {
    pub fn new() -> Self {
        PlayerStateUdp {
            is_in_game: false,
            room_id: String::new(),
            is_owner: false,
            state: EntityStateUdp::default(),
        }
    }
}

impl From<PlayerState> for PlayerStateUdp {
    fn from(state: PlayerState) -> Self {
        PlayerStateUdp {
            is_in_game: state.is_in_game,
            room_id: state.room_id,
            is_owner: state.is_owner,
            state: EntityStateUdp::from(state.state.expect("Failed to get entity state.")),
        }
    }
}

impl Default for PlayerUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerUdp {
    pub fn new() -> Self {
        PlayerUdp {
            player_id: String::new(),
            user_name: String::new(),
            nickname: String::new(),
            password: String::new(),
            join_date: String::new(),
            last_login: String::new(),
            win_count: 0,
            lose_count: 0,
            credits: 0,
            email: String::new(),
            state: PlayerStateUdp::default(),
        }
    }
}

impl From<Player> for PlayerUdp {
    fn from(p: Player) -> Self {
        PlayerUdp {
            player_id: p.player_id,
            user_name: p.user_name,
            nickname: p.nickname,
            password: p.password,
            join_date: p.join_date,
            last_login: p.last_login,
            win_count: p.win_count,
            lose_count: p.lose_count,
            credits: p.credits,
            email: p.email,
            state: PlayerStateUdp::from(p.state.expect("Failed to get player state.")),
        }
    }
}

impl Default for RoomStateUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomStateUdp {
    pub fn new() -> Self {
        RoomStateUdp {
            room_id: String::new(),
            room_name: String::new(),
            current_players: 0,
            max_players: 0,
            started: false,
            message: String::new(),
            players: vec![],
        }
    }
}

impl From<RoomState> for RoomStateUdp {
    fn from(state: RoomState) -> Self {
        RoomStateUdp {
            room_id: state.room_id,
            room_name: state.room_name,
            current_players: state.current_players,
            max_players: state.max_players,
            started: state.started,
            message: state.message,
            players: state
                .players
                .into_iter()
                .map(|p| PlayerUdp::from(p))
                .collect::<Vec<_>>(),
        }
    }
}
