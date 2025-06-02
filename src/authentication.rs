use std::{fs::read_to_string, str, sync::Arc};

use askama::Template;
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    Form, Router,
    extract::{Query, Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{COOKIE, SET_COOKIE, ToStrError},
    },
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use log::{debug, info};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, StandardRevocableToken, TokenResponse, TokenUrl,
    basic::{
        BasicClient, BasicErrorResponse, BasicRevocationErrorResponse,
        BasicTokenIntrospectionResponse, BasicTokenResponse,
    },
};
use rusqlite::{Connection, Result};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::index::render_main;

const SESSION: &str = "session";

#[derive(Clone)]
pub struct User {
    pub name: String,
}

#[derive(Clone)]
pub struct SessionState {
    pub database_connection: Arc<Mutex<Connection>>,
    pub store: MemoryStore,
}

pub async fn extract_session(
    State(state): State<SessionState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Authenticating user session");
    let session = get_user_session(request.headers(), &state.store).await;
    dbg!(&session);
    // TODO: Username should be fetched from db.
    let username = session.and_then(|s| s.get("username"));
    dbg!(&username);
    request.extensions_mut().insert(User {
        name: username.unwrap_or_else(|| "".to_string()),
    });
    Ok(next.run(request).await)
}

pub fn routes(database_connection: Arc<Mutex<Connection>>, store: MemoryStore) -> Router {
    Router::new()
        .route("/authorized", get(authenticate_from_discord))
        .route("/discord", get(redirect_to_discord))
        .route("/create_user", get(get_user))
        .route("/create_user", post(post_user))
        .with_state(SessionState {
            database_connection,
            store,
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
    // TODO: Don't read the secrets from disc all the time probably...
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

// Basic Oath2 flow
//
// 1. User is redirected to Discord.
// 2. User comes back with an authorization code.
// 3. We use that code to request an authorization token from Discord.
// 4. We use the token to get the identity of the user from Discord.

// 1. We redirect the user to Discord where they authorize our app.
async fn redirect_to_discord(
    State(state): State<SessionState>,
) -> Result<impl IntoResponse, Error> {
    // To know that we're the originator of the request when the user comes back from Discord
    let client = create_oauth2_client();
    let (url, token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .add_extra_param("prompt".to_string(), "none".to_string())
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
    let cookie = format!("{SESSION}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().expect("Cookie should be parseable always."),
    );

    info!("Sending user to Discord to authenticate");

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
    State(state): State<SessionState>,
) -> Response {
    debug!("Coming back from discord");
    // FIXME: Validate CSRF token to ensure we originated the request in the first place.

    // 3. We use that code to request an authorization token from Discord.
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Hardcoded client should always build.");
    let client = create_oauth2_client();
    let token = client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(&http_client)
        .await
        .unwrap();

    debug!("Code authenticated");

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

    let conn = state.database_connection.lock().await;
    let name_maybe = conn.query_row(
        "select * from user where discord_id = (?1)",
        (&user_data.id,),
        |row| Ok(row.get::<usize, String>(1).unwrap()),
    );

    dbg!(&name_maybe);

    let cookie = state
        .store
        .store_session(Session::new())
        .await
        .expect("Storing into memory store should never fail.")
        .expect("Should always get a cookie.");
    // Need to get the session out of the store. Creating it outside and cloning it will mess
    // with its id.
    let mut session = state
        .store
        .load_session(cookie.clone())
        .await
        .expect("We just put this session in.")
        .expect("really!");
    let cookie = format!("{SESSION}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().expect("Cookie should be parseable always."),
    );

    if let Err(rusqlite::Error::QueryReturnedNoRows) = name_maybe {
        info!("Redirecting to create new user");
        session
            .insert("discord_id", &user_data.id)
            .expect("MemoryStore should work every time");
        session
            .insert("discord_username", &user_data.username)
            .expect("MemoryStore should work every time");
        return (headers, Redirect::to("create_user")).into_response();
    }

    match name_maybe {
        Ok(name) => {
            info!("Logging in existing user");
            // TODO: This should be just fetched from the user table based on the id instead.
            session.insert("username", name.clone()).unwrap();
            (headers, Redirect::to("/")).into_response()
        }
        // Something went horribly wrong!
        Err(err) => panic!("Unexpected error: {err}"),
    }
}

#[derive(Template)]
#[template(path = "create_user.html")]
struct DisplayUser {
    username: String,
}

async fn get_user_session(headers: &HeaderMap, store: &MemoryStore) -> Option<Session> {
    let cookie = match headers.get(COOKIE)?.to_str() {
        Ok(cookie) => cookie,
        Err(_) => return None,
    };
    // parse coookie
    let kv_pairs = cookie.split(";");
    let session_id = kv_pairs
        .map(|kv_string| {
            let mut kv = kv_string.splitn(2, "=");
            (kv.next().unwrap(), kv.next().unwrap())
        })
        .find_map(|(key, value)| match key {
            SESSION => Some(value),
            _ => None,
        })?;

    dbg!(session_id);

    let session = match store.load_session(session_id.to_string()).await {
        Ok(session) => session,
        // TODO: Clear the cookie here
        Err(_) => return None,
    };

    println!("Fetched session from cookie");
    dbg!(&session);
    session
}

async fn get_user(
    headers: HeaderMap,
    State(state): State<SessionState>,
) -> impl IntoResponse {
    let session = get_user_session(&headers, &state.store).await.unwrap();
    let content = DisplayUser {
        username: session.get("discord_username").unwrap(),
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main("".to_string(), content)
    };
    Html(content)
}

#[derive(Deserialize, Debug)]
struct UserForm {
    username: String,
}

#[derive(Template)]
#[template(path = "welcome_user.html")]
struct WelcomeUser {
    username: String,
}
async fn post_user(
    headers: HeaderMap,
    State(state): State<SessionState>,
    Form(user_form): Form<UserForm>,
) -> impl IntoResponse {
    dbg!(&user_form);
    let mut session = get_user_session(&headers, &state.store).await.unwrap();
    let discord_id: String = session.get("discord_id").unwrap();
    let username = user_form.username;
    let conn = state.database_connection.lock().await;
    conn.execute(
        "insert into user (username, discord_id) values ((?1), (?2))",
        (&username, &discord_id),
    )
    .unwrap();

    session.insert("username", username.clone()).unwrap();

    dbg!(session);

    let content = WelcomeUser {
        username: username.clone(),
    }
    .render()
    .expect("Template rendering should always succeed");
    // Always redraw everything to update log in state.
    Html(render_main(username, content))
}

struct Error {}

impl From<ToStrError> for Error {
    fn from(_: ToStrError) -> Self {
        return Error {};
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        StatusCode::FORBIDDEN.into_response()
    }
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
