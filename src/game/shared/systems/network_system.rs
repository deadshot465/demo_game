use crate::game::shared::structs::Primitive;
use crate::protos::grpc_service::game_state::{
    GetTerrainRequest, Player, RegisterPlayerRequest, RoomState, StartGameRequest,
};
use crate::protos::grpc_service::grpc_service_client::GrpcServiceClient;
use crate::protos::grpc_service::{Empty, LoginRequest, RegisterRequest};
use crate::protos::jwt_token_service::jwt_token_service_client::JwtTokenServiceClient;
use crate::protos::jwt_token_service::AccessRequest;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

//const SERVER_ENDPOINT: &str = "http://64.227.99.31:26361";
const SERVER_ENDPOINT: &str = "http://localhost:26361";

/// ユーザーが入力した内容を検証する正規表現。<br />
/// Regular expressions used to validate user inputs.
static USERNAME_REGEX: OnceCell<Regex> = OnceCell::new();
static EMAIL_REGEX: OnceCell<Regex> = OnceCell::new();

/// サーバーと通信するためのJWTトークン。<br />
/// JWT token used to communicate with server.
#[derive(Deserialize, Serialize)]
struct Authentication {
    pub token: String,
    #[serde(rename = "userDetails")]
    pub user_details: Option<UserDetails>,
    pub expiry: Option<String>,
}

/// トークンと共に戻されたユーザーデータ。<br />
/// User data returned with token.
#[derive(Deserialize, Serialize)]
struct UserDetails {
    #[serde(rename = "userName")]
    pub user_name: String,
    #[serde(rename = "userRole")]
    pub user_role: String,
    #[serde(rename = "type")]
    pub user_type: u8,
}

/// ネットワークを処理する主なシステム。<br />
/// Primary system for handling network.
pub struct NetworkSystem {
    /// プレイヤーが既にログインしている？<br />
    /// Is player already logged in?
    pub is_player_login: bool,

    /// プレイヤーがいられる部屋は必ず一つしかないので、その部屋のステートを保存する。<br />
    /// Since the player can only exist in a room at a given time, we save the room state into this field.
    pub room_state: Arc<Mutex<RoomState>>,

    /// ログインしたプレイヤーのデータ。まだログインしていないならNoneを保存する。<br />
    /// The current logged in player. None if not yet logged in.
    pub logged_user: Option<Player>,

    /// もらったトークンや検証データを保存するためのフィールド。<br />
    /// A field to store acquired JWT token and authentication data.
    authentication: Authentication,

    /// JWTトークンについては異なっているサービスが使われているので、違うクライアントも必要です。<br />
    /// We use another different gRPC service for JWT token, so we also need another client.
    jwt_client: JwtTokenServiceClient<tonic::transport::Channel>,

    /// ゲームデータの転送・取得を処理する主なgRPCクライアント。<br />
    /// Primary gRPC client for sending and receiving game data.
    grpc_client: GrpcServiceClient<tonic::transport::Channel>,
}

/// ネットワークシステムの実装
impl NetworkSystem {
    ///　コンストラクター。<br />
    /// Constructor.
    pub async fn new() -> anyhow::Result<Self> {
        let mut jwt_client = JwtTokenServiceClient::connect(SERVER_ENDPOINT).await?;
        let grpc_client = GrpcServiceClient::connect(SERVER_ENDPOINT).await?;
        let authentication = Self::authenticate(&mut jwt_client).await?;

        // 無効な入力は禁止されているので正規表現で検証する。<br />
        // Invalid inputs are not allowed, so we use regular expression to validate them.
        USERNAME_REGEX
            .set(Regex::new(r".").expect("Failed to initialize regular expression."))
            .expect("Failed to initialize regular expression.");
        EMAIL_REGEX
            .set(
                Regex::new(r"([a-zA-Z0-9._]+)@{1}([a-zA-Z0-9._]+)")
                    .expect("Failed to initialize regular expression."),
            )
            .expect("Failed to initialize regular expression.");

        Ok(NetworkSystem {
            authentication,
            is_player_login: false,
            logged_user: None,
            jwt_client,
            grpc_client,
            // 部屋のデータはサーバーから取得するため、ここで一旦初期化する。<br />
            // We will get room data from the server, so we initialize it first.
            room_state: Arc::new(Mutex::new(RoomState {
                room_id: String::new(),
                room_name: String::new(),
                current_players: 0,
                max_players: 0,
                started: false,
                players: vec![],
                message: String::new(),
            })),
        })
    }

    /// 既存の部屋を全て取得する。<br />
    /// Retrieve all existing rooms from server.
    pub async fn get_rooms(&mut self) -> anyhow::Result<Vec<RoomState>> {
        let request = tonic::Request::new(Empty {});
        let response = self.grpc_client.get_rooms(request).await?;
        let response = response.into_inner();
        Ok(response.rooms)
    }

    /// 地形の頂点、インデックスなどを取得する。<br />
    /// 同じ部屋なら必ず地形を統一化しないといけませんので、ホスト（部屋を作るプレイヤー）のパソコンで地形を生成した後、<br />
    /// サーバーに転送し、そしてサーバーがその地形のデータを同じ部屋にいる他のプレイヤーに配るという形で実現する。<br />
    /// Retrieve vertices and indices of a terrain.<br />
    /// All players must see and exist on the same terrain if they are in the same room, so the host's computer will generate the terrain first.<br />
    /// The terrain then will be sent to the server, and the server will broadcast that terrain to all other players in the same room.
    pub async fn get_terrain(&mut self) -> anyhow::Result<Primitive> {
        let request = tonic::Request::new(GetTerrainRequest {
            room_id: self.room_state.lock().await.room_id.clone(),
        });

        let response = self.grpc_client.get_terrain(request).await?;
        let response = response.into_inner();
        let primitive = serde_json::from_slice::<Primitive>(&response.terrain_vertices)?;
        Ok(primitive)
    }

    ///　登録した使用者のデータ、もしくは入力された既存のデータでログインする。<br />
    /// Using registered player's data or inputted data to login player.
    pub async fn login(&mut self, login_data: Option<(String, String)>) -> Option<Player> {
        if let Some((account, password)) = login_data {
            let request = tonic::Request::new(LoginRequest {
                account,
                password,
                jwt_token: self.authentication.token.clone(),
            });
            let response = self
                .grpc_client
                .login(request)
                .await
                .expect("Failed to get login reply.");
            let mut response = response.into_inner();
            if response.status {
                let player = response
                    .player
                    .take()
                    .expect("Failed to get player from response.");
                self.logged_user = Some(player);
                self.is_player_login = true;
                self.logged_user.clone()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// ユーザーが入力したデータに基づいてサーバーとデータベースに登録する。<br />
    /// Register player to the database and server using inputted information.
    pub async fn register(
        &mut self,
        username: &str,
        nickname: &str,
        email: &str,
        password: &str,
    ) -> (bool, Option<Player>) {
        if !Self::verify(username, nickname, email, password) {
            (false, None)
        } else {
            let encoded_pass = base64::encode(password.trim());
            let request = tonic::Request::new(RegisterRequest {
                user_name: username.trim().to_string(),
                nickname: nickname.trim().to_string(),
                email: email.trim().to_string(),
                password: encoded_pass.clone(),
                jwt_token: self.authentication.token.clone(),
            });

            let response = self
                .grpc_client
                .register(request)
                .await
                .expect("Failed to register against the server.");

            let response = response.into_inner();
            if response.status {
                if let Some(player) = self.login(Some((username.to_string(), encoded_pass))).await {
                    (true, Some(player))
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            }
        }
    }

    /// プレイヤーを部屋に登録する。<br />
    /// 部屋が存在していないなら新しい部屋を作る。<br />
    /// Register player to a room.<br />
    /// If the room doesn't exist yet, create a new room.
    pub async fn register_player(
        &mut self,
        room_id: String,
        room_name: String,
        is_owner: bool,
    ) -> anyhow::Result<crossbeam::channel::Receiver<bool>> {
        if let Some(player) = self.logged_user.as_mut() {
            if let Some(state) = player.state.as_mut() {
                state.is_owner = is_owner;
                state.room_id = room_id.to_string();
            }
        }
        let request = tonic::Request::new(RegisterPlayerRequest {
            room_id,
            room_name,
            player: self.logged_user.clone(),
        });
        let response = self.grpc_client.register_player(request).await?;
        let response = response.into_inner();
        let room_state = self.room_state.clone();
        let (send, recv) = crossbeam::channel::bounded(5);
        tokio::spawn(async {
            let current_room_state = room_state;
            let mut response = response;
            let send = send;
            while let Ok(r) = response.message().await {
                let mut state = current_room_state.lock().await;
                if state.started {
                    send.send(true)
                        .expect("Failed to send room state to main thread.");
                    break;
                }
                if let Some(actual_state) = r {
                    *state = actual_state;
                }
            }
        });
        Ok(recv)
    }

    /// 部屋を待たせないようにして、ゲームを始める。<br />
    /// この関数を呼び出せるのはホスト（部屋のオーナー）のみです。<br />
    /// Stop waiting in a room and start the game.<br />
    /// This function can only be invoked by the client of the host (the owner of the room).
    pub async fn start_game(&mut self, primitive: Primitive) -> anyhow::Result<()> {
        let serialized_data = serde_json::to_vec(&primitive)?;
        let request = tonic::Request::new(StartGameRequest {
            room_state: Some(self.room_state.lock().await.clone()),
            terrain_vertices: serialized_data,
        });
        self.grpc_client.start_game(request).await?;
        Ok(())
    }

    /// サーバーと通信するためのJWTトークンを取得する。<br />
    /// Retrieve JWT token for communication with server.
    async fn authenticate(
        client: &mut JwtTokenServiceClient<tonic::transport::Channel>,
    ) -> anyhow::Result<Authentication> {
        let request = tonic::Request::new(AccessRequest {
            user_name: dotenv::var("LOGIN_NAME")?,
            password: dotenv::var("LOGIN_PASS")?,
        });

        let response = client.access(request).await?;
        let mut response = response.into_inner();
        let user_details = response
            .user_details
            .take()
            .expect("Failed to get user detail from gRPC response.");
        Ok(Authentication {
            token: response.token,
            user_details: Some(UserDetails {
                user_name: user_details.user_name,
                user_role: user_details.user_role,
                user_type: user_details.r#type as u8,
            }),
            expiry: Some(response.expiry),
        })
    }

    /// 正規表現により入力されたデータを検証する。<br />
    /// Verify user inputs using regular expressions.
    fn verify(username: &str, nickname: &str, email: &str, password: &str) -> bool {
        let names_valid = if let Some(regex) = USERNAME_REGEX.get() {
            regex.is_match(username) && regex.is_match(nickname) && regex.is_match(password)
        } else {
            false
        };
        let email_valid = if let Some(regex) = EMAIL_REGEX.get() {
            regex.is_match(email)
        } else {
            false
        };
        names_valid && email_valid
    }
}
