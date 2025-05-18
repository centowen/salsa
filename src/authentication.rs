use std::fs::read_to_string;

use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    Router,
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, StandardRevocableToken, TokenResponse, TokenUrl,
    basic::{
        BasicClient, BasicErrorResponse, BasicRevocationErrorResponse,
        BasicTokenIntrospectionResponse, BasicTokenResponse,
    },
};
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/authorized", get(authenticate_from_discord))
        .route("/discord", get(redirect_to_discord))
        .with_state(AuthenticationState {
            store: MemoryStore::new(),
            client: create_oauth2_client(),
        })
}

// This type is specified because the oauth2 crate uses type states (it encodes
// the state within the type). Perhaps there is a more convenient way to do this?
pub type DiscordClient<
    HasAuthUrl = EndpointSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointSet,
> = oauth2::Client<
    BasicErrorResponse,
    BasicTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
>;

#[derive(Deserialize)]
struct Secrets {
    discord: ClientSecrets,
}

#[derive(Deserialize)]
struct ClientSecrets {
    client_id: String,
    client_secret: String,
}

fn read_discord_secrets() -> ClientSecrets {
    let contents = read_to_string(".secrets.toml").expect("A .secrets.toml file must exist.");
    let secrets: Secrets =
        toml::from_str(&contents).expect("The .secrets.toml file should be parseable");
    secrets.discord
}

fn create_oauth2_client() -> DiscordClient {
    let ClientSecrets {
        client_id,
        client_secret,
    } = read_discord_secrets();
    BasicClient::new(ClientId::new(client_id.to_string()))
        .set_client_secret(ClientSecret::new(client_secret.to_string()))
        .set_auth_uri(
            AuthUrl::new("https://discord.com/api/oauth2/authorize?response_type=code".to_string())
                .expect("Hardcoded URL should always work"),
        )
        // FIXME: This url should be retrieved from where we are deployed.
        .set_redirect_uri(
            RedirectUrl::new("http://127.0.0.1:3000/auth/authorized".to_string())
                .expect("Hardcoded URL should always work."),
        )
        .set_token_uri(
            TokenUrl::new("https://discord.com/api/oauth2/token".to_string())
                .expect("Hardcoded URL should always work."),
        )
}

#[derive(Clone)]
struct AuthenticationState {
    store: MemoryStore,
    client: DiscordClient,
}

struct Error {}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        todo!()
    }
}

// Basic Oath2 flow
//
// 1. User is redirected to Discord.
// 2. User comes back with an authorization code.
// 3. We use that code to request an authorization token from Discord.
// 4. We use the token to get the identity of the user from Discord.

// 1. We redirect the user to Discord where they authorize our app.
async fn redirect_to_discord(
    State(state): State<AuthenticationState>,
) -> Result<impl IntoResponse, Error> {
    // To know that we're the originator of the request when the user comes back from Discord
    let (url, token) = state
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .url();

    let mut session = Session::new();
    session
        .insert("csrf_token", &token)
        .expect("Data created entirely by us");
    let cookie = state
        .store
        .store_session(session)
        .await
        .expect("Storing into memory store should never fail.")
        .expect("Should always get a cookie.");
    let cookie = format!("session={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().expect("Cookie should be parseable always."),
    );

    Ok((headers, Redirect::to(url.as_ref())))
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    avatar: Option<String>,
    username: String,
    discriminator: String,
}

// 2. User comes back with an authorization code.
async fn authenticate_from_discord(
    Query(query): Query<AuthRequest>,
    State(state): State<AuthenticationState>,
) -> impl IntoResponse {
    // FIXME: Validate CSRF token to ensure we originated the request in the first place.

    // 3. We use that code to request an authorization token from Discord.
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Hardcoded client should always build.");
    let token = state
        .client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(&http_client)
        .await
        .unwrap();

    // 4. We use the token to get the identity of the user from Discord.
    let user_data: DiscordUser = http_client
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .unwrap()
        .json::<DiscordUser>()
        .await
        .unwrap();

    // TODO: Set up our user state and session. Ask the user for a display name.

    Html(format!("{:?}", user_data))
}

#[derive(Clone)]
pub struct User {
    pub name: String,
}

// TODO: Can be made into extractor only I think?
pub async fn extract_session(mut request: Request, next: Next) -> Result<Response, StatusCode> {
    // TODO: Insert real authentication here.
    request.extensions_mut().insert(User {
        name: String::from("frood"),
    });
    Ok(next.run(request).await)
}

// OAuth2 based authentication
//
// - Follow the Axum example to set up one provider.
// - We need session storage or jwt.
//   - The async-session crate could be used, but then we need sqlx probably.
// - Set up more providers
//   - Google
//   - Apple
//   - Microsoft?
//   - Some european alternative?
// - The user still needs to live in our database, it's just that auth goes through OAuth2
//   - Pseudonymous should be allowed
//
