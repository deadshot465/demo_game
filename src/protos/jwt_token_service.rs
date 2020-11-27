#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccessRequest {
    #[prost(string, tag = "1")]
    pub user_name: std::string::String,
    #[prost(string, tag = "2")]
    pub password: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccessReply {
    #[prost(string, tag = "1")]
    pub token: std::string::String,
    #[prost(message, optional, tag = "2")]
    pub user_details: ::std::option::Option<access_reply::User>,
    #[prost(string, tag = "3")]
    pub expiry: std::string::String,
}
pub mod access_reply {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct User {
        #[prost(enumeration = "user::UserType", tag = "1")]
        pub r#type: i32,
        #[prost(string, tag = "2")]
        pub user_name: std::string::String,
        #[prost(string, tag = "3")]
        pub user_role: std::string::String,
        #[prost(string, tag = "4")]
        pub password: std::string::String,
    }
    pub mod user {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum UserType {
            Admin = 0,
            Bot = 1,
        }
    }
}
#[doc = r" Generated client implementations."]
pub mod jwt_token_service_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct JwtTokenServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl JwtTokenServiceClient<tonic::transport::Channel> {
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
    impl<T> JwtTokenServiceClient<T>
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
        #[doc = " Access the server and acquire a JWT token."]
        pub async fn access(
            &mut self,
            request: impl tonic::IntoRequest<super::AccessRequest>,
        ) -> Result<tonic::Response<super::AccessReply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/jwt_token_service.JwtTokenService/Access");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for JwtTokenServiceClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for JwtTokenServiceClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "JwtTokenServiceClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod jwt_token_service_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with JwtTokenServiceServer."]
    #[async_trait]
    pub trait JwtTokenService: Send + Sync + 'static {
        #[doc = " Access the server and acquire a JWT token."]
        async fn access(
            &self,
            request: tonic::Request<super::AccessRequest>,
        ) -> Result<tonic::Response<super::AccessReply>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct JwtTokenServiceServer<T: JwtTokenService> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: JwtTokenService> JwtTokenServiceServer<T> {
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
    impl<T, B> Service<http::Request<B>> for JwtTokenServiceServer<T>
    where
        T: JwtTokenService,
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
                "/jwt_token_service.JwtTokenService/Access" => {
                    #[allow(non_camel_case_types)]
                    struct AccessSvc<T: JwtTokenService>(pub Arc<T>);
                    impl<T: JwtTokenService> tonic::server::UnaryService<super::AccessRequest> for AccessSvc<T> {
                        type Response = super::AccessReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AccessRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).access(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = AccessSvc(inner);
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
    impl<T: JwtTokenService> Clone for JwtTokenServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: JwtTokenService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: JwtTokenService> tonic::transport::NamedService for JwtTokenServiceServer<T> {
        const NAME: &'static str = "jwt_token_service.JwtTokenService";
    }
}
