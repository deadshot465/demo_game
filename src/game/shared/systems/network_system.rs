use crate::game::shared::structs::Player;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync;

const LOGIN_NAME: &str = "bot";
const LOGIN_PASS: &str = "kf0TmiW2ABxfm89QLxyAlXCa1opDzt";

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
    client: reqwest::Client,
}

impl NetworkSystem {
    pub async fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::new();
        let authentication = Self::authenticate(&client).await?;

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
            client,
            is_player_login: false,
            logged_user: None,
        })
    }

    pub async fn login(&mut self, login_data: Option<(String, String)>) -> Option<Player> {
        if let Some((account, password)) = login_data {
            let mut request_data = HashMap::new();
            request_data.insert("Account", account.trim());
            request_data.insert("Password", password.trim());
            let token = self.authentication.token.clone();
            let resp = self
                .client
                .post("https://tetsukizone.com/api/player/login")
                .bearer_auth(&token)
                .json(&request_data)
                .header("Content-Type", "application/json")
                .send()
                .await
                .expect("Failed to login player.");
            let status = resp.status();
            if !status.is_success() {
                None
            } else {
                let player: Player = resp
                    .json()
                    .await
                    .expect("Failed to convert response to JSON.");
                self.logged_user = Some(player);
                self.logged_user.clone()
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
            let handle = tokio::runtime::Handle::current();
            let client = self.client.clone();
            let username = username.to_string();
            let nickname = nickname.to_string();
            let email = email.to_string();
            let password = password.to_string();
            let token = self.authentication.token.clone();
            let result = handle.block_on(async {
                let client = client;
                let mut request_data = HashMap::new();
                request_data.insert("UserName", username.trim());
                request_data.insert("Nickname", nickname.trim());
                request_data.insert("Email", email.trim());
                let encoded_pass = base64::encode(password.trim());
                request_data.insert("UserName", &encoded_pass);
                let resp = client
                    .post("https://tetsukizone.com/api/player/register")
                    .json(&request_data)
                    .bearer_auth(token)
                    .header("Content-Type", "application/json")
                    .send()
                    .await
                    .expect("Failed to register against the server.");
                if resp.status().is_success() {
                    Some((username, encoded_pass))
                } else {
                    None
                }
            });
            if let Some(player) = self.login(result).await {
                (true, Some(player))
            } else {
                (false, None)
            }
        }
    }

    async fn authenticate(client: &reqwest::Client) -> anyhow::Result<Authentication> {
        let mut login_data = HashMap::new();
        login_data.insert("UserName", LOGIN_NAME);
        login_data.insert("Password", LOGIN_PASS);
        let response = client
            .post("https://tetsukizone.com/api/login")
            .json(&login_data)
            .send()
            .await?;
        let resp: Authentication = response.json().await?;
        Ok(resp)
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
