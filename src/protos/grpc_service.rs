#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterRequest {
    #[prost(string, tag = "1")]
    pub user_name: std::string::String,
    #[prost(string, tag = "2")]
    pub nickname: std::string::String,
    #[prost(string, tag = "3")]
    pub email: std::string::String,
    #[prost(string, tag = "4")]
    pub password: std::string::String,
    #[prost(string, tag = "5")]
    pub jwt_token: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterReply {
    #[prost(bool, tag = "1")]
    pub status: bool,
    #[prost(string, tag = "2")]
    pub message: std::string::String,
    #[prost(message, optional, tag = "3")]
    pub player: ::std::option::Option<game_state::Player>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoginRequest {
    #[prost(string, tag = "1")]
    pub account: std::string::String,
    #[prost(string, tag = "2")]
    pub password: std::string::String,
    #[prost(string, tag = "3")]
    pub jwt_token: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoginReply {
    #[prost(bool, tag = "1")]
    pub status: bool,
    #[prost(string, tag = "2")]
    pub message: std::string::String,
    #[prost(message, optional, tag = "3")]
    pub player: ::std::option::Option<game_state::Player>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MessageRecord {
    #[prost(string, tag = "1")]
    pub player_id: std::string::String,
    #[prost(string, tag = "2")]
    pub message: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IncomingMessages {
    #[prost(message, repeated, tag = "1")]
    pub messages: ::std::vec::Vec<IncomingMessage>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IncomingMessage {
    #[prost(string, tag = "1")]
    pub author: std::string::String,
    #[prost(string, tag = "2")]
    pub message: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Empty {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameState {}
pub mod game_state {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Player {
        #[prost(string, tag = "1")]
        pub player_id: std::string::String,
        #[prost(string, tag = "2")]
        pub user_name: std::string::String,
        #[prost(string, tag = "3")]
        pub nickname: std::string::String,
        /// Base64-encoded password
        #[prost(string, tag = "4")]
        pub password: std::string::String,
        #[prost(string, tag = "5")]
        pub join_date: std::string::String,
        #[prost(string, tag = "6")]
        pub last_login: std::string::String,
        #[prost(int32, tag = "7")]
        pub win_count: i32,
        #[prost(int32, tag = "8")]
        pub lose_count: i32,
        #[prost(int32, tag = "9")]
        pub credits: i32,
        #[prost(string, tag = "10")]
        pub email: std::string::String,
        #[prost(message, optional, tag = "11")]
        pub state: ::std::option::Option<PlayerState>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct EntityState {
        #[prost(int32, tag = "1")]
        pub current_hp: i32,
        #[prost(int32, tag = "2")]
        pub max_hp: i32,
        #[prost(int32, tag = "3")]
        pub current_sp: i32,
        #[prost(int32, tag = "4")]
        pub max_sp: i32,
        #[prost(bool, tag = "5")]
        pub is_alive: bool,
        #[prost(float, repeated, tag = "6")]
        pub world_matrix: ::std::vec::Vec<f32>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct PlayerState {
        #[prost(bool, tag = "1")]
        pub is_in_game: bool,
        #[prost(string, tag = "2")]
        pub room_id: std::string::String,
        #[prost(bool, tag = "3")]
        pub is_owner: bool,
        #[prost(message, optional, tag = "4")]
        pub state: ::std::option::Option<EntityState>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Rooms {
        #[prost(message, repeated, tag = "1")]
        pub rooms: ::std::vec::Vec<RoomState>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RegisterPlayerRequest {
        #[prost(string, tag = "1")]
        pub room_id: std::string::String,
        #[prost(string, tag = "2")]
        pub room_name: std::string::String,
        #[prost(message, optional, tag = "3")]
        pub player: ::std::option::Option<Player>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RoomState {
        #[prost(string, tag = "1")]
        pub room_id: std::string::String,
        #[prost(string, tag = "2")]
        pub room_name: std::string::String,
        #[prost(int32, tag = "3")]
        pub current_players: i32,
        #[prost(int32, tag = "4")]
        pub max_players: i32,
        #[prost(bool, tag = "5")]
        pub started: bool,
        #[prost(message, repeated, tag = "6")]
        pub players: ::std::vec::Vec<Player>,
        #[prost(string, tag = "7")]
        pub message: std::string::String,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct StartGameRequest {
        #[prost(message, optional, tag = "1")]
        pub room_state: ::std::option::Option<RoomState>,
        #[prost(bytes, tag = "2")]
        pub terrain_vertices: std::vec::Vec<u8>,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct GetTerrainRequest {
        #[prost(string, tag = "1")]
        pub room_id: std::string::String,
    }
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct GetTerrainReply {
        #[prost(bytes, tag = "1")]
        pub terrain_vertices: std::vec::Vec<u8>,
    }
}
#[doc = r" Generated client implementations."]
pub mod grpc_service_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct GrpcServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl GrpcServiceClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> GrpcServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        #[doc = " Register player to the database."]
        pub async fn register(
            &mut self,
            request: impl tonic::IntoRequest<super::RegisterRequest>,
        ) -> Result<tonic::Response<super::RegisterReply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/Register");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Login player to the chatroom and the server."]
        #[doc = " Ideally once logged in, the gRPC connection will be kept alive until the client shuts down."]
        pub async fn login(
            &mut self,
            request: impl tonic::IntoRequest<super::LoginRequest>,
        ) -> Result<tonic::Response<super::LoginReply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/Login");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Get last 50 messages from the database."]
        pub async fn get_chat_history(
            &mut self,
            request: impl tonic::IntoRequest<super::Empty>,
        ) -> Result<tonic::Response<super::IncomingMessages>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/GetChatHistory");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Send a message to the server and broadcast the message to all connected clients."]
        pub async fn chat(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::MessageRecord>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<super::IncomingMessage>>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/Chat");
            self.inner
                .streaming(request.into_streaming_request(), path, codec)
                .await
        }
        #[doc = " Get all available playrooms."]
        pub async fn get_rooms(
            &mut self,
            request: impl tonic::IntoRequest<super::Empty>,
        ) -> Result<tonic::Response<super::game_state::Rooms>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/GetRooms");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Register player to the room."]
        pub async fn register_player(
            &mut self,
            request: impl tonic::IntoRequest<super::game_state::RegisterPlayerRequest>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::game_state::RoomState>>,
            tonic::Status,
        > {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/RegisterPlayer");
            self.inner
                .server_streaming(request.into_request(), path, codec)
                .await
        }
        #[doc = " Start a game in a room."]
        pub async fn start_game(
            &mut self,
            request: impl tonic::IntoRequest<super::game_state::StartGameRequest>,
        ) -> Result<tonic::Response<super::Empty>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/StartGame");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Get terrain of a game room."]
        pub async fn get_terrain(
            &mut self,
            request: impl tonic::IntoRequest<super::game_state::GetTerrainRequest>,
        ) -> Result<tonic::Response<super::game_state::GetTerrainReply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/GetTerrain");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Progress the game."]
        pub async fn progress_game(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::game_state::RoomState>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::game_state::RoomState>>,
            tonic::Status,
        > {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/grpc_service.GrpcService/ProgressGame");
            self.inner
                .streaming(request.into_streaming_request(), path, codec)
                .await
        }
    }
    impl<T: Clone> Clone for GrpcServiceClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for GrpcServiceClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "GrpcServiceClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod grpc_service_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with GrpcServiceServer."]
    #[async_trait]
    pub trait GrpcService: Send + Sync + 'static {
        #[doc = " Register player to the database."]
        async fn register(
            &self,
            request: tonic::Request<super::RegisterRequest>,
        ) -> Result<tonic::Response<super::RegisterReply>, tonic::Status>;
        #[doc = " Login player to the chatroom and the server."]
        #[doc = " Ideally once logged in, the gRPC connection will be kept alive until the client shuts down."]
        async fn login(
            &self,
            request: tonic::Request<super::LoginRequest>,
        ) -> Result<tonic::Response<super::LoginReply>, tonic::Status>;
        #[doc = " Get last 50 messages from the database."]
        async fn get_chat_history(
            &self,
            request: tonic::Request<super::Empty>,
        ) -> Result<tonic::Response<super::IncomingMessages>, tonic::Status>;
        #[doc = "Server streaming response type for the Chat method."]
        type ChatStream: Stream<Item = Result<super::IncomingMessage, tonic::Status>>
            + Send
            + Sync
            + 'static;
        #[doc = " Send a message to the server and broadcast the message to all connected clients."]
        async fn chat(
            &self,
            request: tonic::Request<tonic::Streaming<super::MessageRecord>>,
        ) -> Result<tonic::Response<Self::ChatStream>, tonic::Status>;
        #[doc = " Get all available playrooms."]
        async fn get_rooms(
            &self,
            request: tonic::Request<super::Empty>,
        ) -> Result<tonic::Response<super::game_state::Rooms>, tonic::Status>;
        #[doc = "Server streaming response type for the RegisterPlayer method."]
        type RegisterPlayerStream: Stream<Item = Result<super::game_state::RoomState, tonic::Status>>
            + Send
            + Sync
            + 'static;
        #[doc = " Register player to the room."]
        async fn register_player(
            &self,
            request: tonic::Request<super::game_state::RegisterPlayerRequest>,
        ) -> Result<tonic::Response<Self::RegisterPlayerStream>, tonic::Status>;
        #[doc = " Start a game in a room."]
        async fn start_game(
            &self,
            request: tonic::Request<super::game_state::StartGameRequest>,
        ) -> Result<tonic::Response<super::Empty>, tonic::Status>;
        #[doc = " Get terrain of a game room."]
        async fn get_terrain(
            &self,
            request: tonic::Request<super::game_state::GetTerrainRequest>,
        ) -> Result<tonic::Response<super::game_state::GetTerrainReply>, tonic::Status>;
        #[doc = "Server streaming response type for the ProgressGame method."]
        type ProgressGameStream: Stream<Item = Result<super::game_state::RoomState, tonic::Status>>
            + Send
            + Sync
            + 'static;
        #[doc = " Progress the game."]
        async fn progress_game(
            &self,
            request: tonic::Request<tonic::Streaming<super::game_state::RoomState>>,
        ) -> Result<tonic::Response<Self::ProgressGameStream>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct GrpcServiceServer<T: GrpcService> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: GrpcService> GrpcServiceServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for GrpcServiceServer<T>
    where
        T: GrpcService,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/grpc_service.GrpcService/Register" => {
                    #[allow(non_camel_case_types)]
                    struct RegisterSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService> tonic::server::UnaryService<super::RegisterRequest> for RegisterSvc<T> {
                        type Response = super::RegisterReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RegisterRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).register(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = RegisterSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/Login" => {
                    #[allow(non_camel_case_types)]
                    struct LoginSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService> tonic::server::UnaryService<super::LoginRequest> for LoginSvc<T> {
                        type Response = super::LoginReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::LoginRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).login(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = LoginSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/GetChatHistory" => {
                    #[allow(non_camel_case_types)]
                    struct GetChatHistorySvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService> tonic::server::UnaryService<super::Empty> for GetChatHistorySvc<T> {
                        type Response = super::IncomingMessages;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(&mut self, request: tonic::Request<super::Empty>) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_chat_history(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = GetChatHistorySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/Chat" => {
                    #[allow(non_camel_case_types)]
                    struct ChatSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService> tonic::server::StreamingService<super::MessageRecord> for ChatSvc<T> {
                        type Response = super::IncomingMessage;
                        type ResponseStream = T::ChatStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::MessageRecord>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).chat(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = ChatSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/GetRooms" => {
                    #[allow(non_camel_case_types)]
                    struct GetRoomsSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService> tonic::server::UnaryService<super::Empty> for GetRoomsSvc<T> {
                        type Response = super::game_state::Rooms;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(&mut self, request: tonic::Request<super::Empty>) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_rooms(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = GetRoomsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/RegisterPlayer" => {
                    #[allow(non_camel_case_types)]
                    struct RegisterPlayerSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService>
                        tonic::server::ServerStreamingService<
                            super::game_state::RegisterPlayerRequest,
                        > for RegisterPlayerSvc<T>
                    {
                        type Response = super::game_state::RoomState;
                        type ResponseStream = T::RegisterPlayerStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::game_state::RegisterPlayerRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).register_player(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = RegisterPlayerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/StartGame" => {
                    #[allow(non_camel_case_types)]
                    struct StartGameSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService>
                        tonic::server::UnaryService<super::game_state::StartGameRequest>
                        for StartGameSvc<T>
                    {
                        type Response = super::Empty;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::game_state::StartGameRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).start_game(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = StartGameSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/GetTerrain" => {
                    #[allow(non_camel_case_types)]
                    struct GetTerrainSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService>
                        tonic::server::UnaryService<super::game_state::GetTerrainRequest>
                        for GetTerrainSvc<T>
                    {
                        type Response = super::game_state::GetTerrainReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::game_state::GetTerrainRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_terrain(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = GetTerrainSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/grpc_service.GrpcService/ProgressGame" => {
                    #[allow(non_camel_case_types)]
                    struct ProgressGameSvc<T: GrpcService>(pub Arc<T>);
                    impl<T: GrpcService>
                        tonic::server::StreamingService<super::game_state::RoomState>
                        for ProgressGameSvc<T>
                    {
                        type Response = super::game_state::RoomState;
                        type ResponseStream = T::ProgressGameStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::game_state::RoomState>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).progress_game(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = ProgressGameSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: GrpcService> Clone for GrpcServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: GrpcService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: GrpcService> tonic::transport::NamedService for GrpcServiceServer<T> {
        const NAME: &'static str = "grpc_service.GrpcService";
    }
}
