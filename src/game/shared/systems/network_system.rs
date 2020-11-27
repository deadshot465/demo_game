use crate::protos::grpc_service::game_state::Player;
use crate::protos::grpc_service::grpc_service_client::GrpcServiceClient;
use crate::protos::grpc_service::{LoginRequest, RegisterRequest};
use crate::protos::jwt_token_service::jwt_token_service_client::JwtTokenServiceClient;
use crate::protos::jwt_token_service::AccessRequest;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync;

static USERNAME_REGEX: OnceCell<Regex> = OnceCell::new();
static EMAIL_REGEX: OnceCell<Regex> = OnceCell::new();

#[derive(Deserialize, Serialize)]
struct Authentication {
    pub token: String,
    #[serde(rename = "userDetails")]
    pub user_details: Option<UserDetails>,
    pub expiry: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct UserDetails {
    #[serde(rename = "userName")]
    pub user_name: String,
    #[serde(rename = "userRole")]
    pub user_role: String,
    #[serde(rename = "type")]
    pub user_type: u8,
}

pub struct NetworkSystem {
    pub is_player_login: bool,
    authentication: Authentication,
    logged_user: Option<Player>,
    jwt_client: JwtTokenServiceClient<tonic::transport::Channel>,
    grpc_client: GrpcServiceClient<tonic::transport::Channel>,
}

impl NetworkSystem {
    pub async fn new() -> anyhow::Result<Self> {
        let mut jwt_client = JwtTokenServiceClient::connect("http://64.227.99.31:26361").await?;
        let grpc_client = GrpcServiceClient::connect("http://64.227.99.31:26361").await?;
        let authentication = Self::authenticate(&mut jwt_client).await?;

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
        })
    }

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
                self.logged_user.clone()
            } else {
                None
            }
        } else {
            None
        }
    }

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
