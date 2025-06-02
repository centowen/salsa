use std::{fs::read_to_string, str, sync::Arc};

use askama::Template;
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    Form, Router,
    extract::{Query, Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use log::{debug, error, info};
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

const SESSION_COOKIE_NAME: &str = "session";

#[derive(Clone)]
pub struct AuthenticationState {
    pub database_connection: Arc<Mutex<Connection>>,
    pub store: MemoryStore,
}

#[derive(Clone)]
pub struct User {
    pub name: String,
}

pub async fn extract_session(
    State(state): State<AuthenticationState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Authenticating user session");
    let session = get_user_session(request.headers(), &state.store).await;
    // TODO(#151): Username should be fetched from db.
    let username = session.and_then(|s| s.get("username"));
    request.extensions_mut().insert(User {
        name: username.unwrap_or_else(|| "".to_string()),
    });
    debug!("User authenticated");
    Ok(next.run(request).await)
}

pub fn routes(database_connection: Arc<Mutex<Connection>>, store: MemoryStore) -> Router {
    Router::new()
        .route("/authorized", get(authenticate_from_discord))
        .route("/discord", get(redirect_to_discord))
        .route("/create_user", get(get_user))
        .route("/create_user", post(post_user))
        .with_state(AuthenticationState {
            database_connection,
            store,
        })
}

#[derive(Deserialize)]
struct Secrets {
    discord: ClientSecrets,
}

#[derive(Deserialize)]
struct ClientSecrets {
    client_id: String,
    client_secret: String,
}

fn read_discord_secrets() -> Result<ClientSecrets, InternalError> {
    let filename = ".secrets.toml";
    let contents = read_to_string(filename)
        .map_err(|err| InternalError::new(format!("Failed to read from '{filename}': {err}")))?;
    let secrets: Secrets = toml::from_str(&contents).map_err(|err| {
        InternalError::new(format!("Failed to parse toml from {filename}: {err}"))
    })?;
    Ok(secrets.discord)
}

fn create_oauth2_client() -> Result<DiscordClient, InternalError> {
    // TODO: Don't read the secrets from disc all the time probably...
    let ClientSecrets {
        client_id,
        client_secret,
    } = read_discord_secrets()?;
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_client_secret(ClientSecret::new(client_secret.to_string()))
        .set_auth_uri(
            AuthUrl::new("https://discord.com/api/oauth2/authorize?response_type=code".to_string())
                .expect("Hardcoded URL should always work"),
        )
        // TODO: This url should be retrieved from where we are deployed.
        .set_redirect_uri(
            RedirectUrl::new("http://127.0.0.1:3000/auth/authorized".to_string())
                .expect("Hardcoded URL should always work."),
        )
        .set_token_uri(
            TokenUrl::new("https://discord.com/api/oauth2/token".to_string())
                .expect("Hardcoded URL should always work."),
        );
    Ok(client)
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
) -> Result<impl IntoResponse, InternalError> {
    // To know that we're the originator of the request when the user comes back from Discord
    let (url, token) = create_oauth2_client()?
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
    let cookie = format!("{SESSION_COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

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
    #[allow(dead_code)] // TODO(#152): Use the state to get CSRF token.
    state: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    #[allow(dead_code)] // We don't use this.
    avatar: Option<String>,
    username: String,
    #[allow(dead_code)] // We don't use this.
    discriminator: String,
}

// 2. User comes back with an authorization code.
async fn authenticate_from_discord(
    Query(query): Query<AuthRequest>,
    State(state): State<AuthenticationState>,
) -> Result<Response, InternalError> {
    debug!("Coming back from Discord");
    // TODO(#152): Validate CSRF token to ensure we originated the request in the first place.

    // 3. We use that code to request an authorization token from Discord.
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Hardcoded client should always build.");
    let token = match create_oauth2_client()?
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(&http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            debug!("Failed to get token from Discord: {err}");
            // This realistically happens because of an old or bogus request
            // to this endpoint. Returning 401 is reasonable.
            return Ok(StatusCode::UNAUTHORIZED.into_response());
        }
    };

    debug!("Code authenticated");

    // 4. We use the token to get the identity of the user from Discord.
    let user_data: DiscordUser = http_client
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|err| InternalError::new(format!("Failed to fetch token from Discord: {err}")))?
        .json::<DiscordUser>()
        .await
        .map_err(|err| {
            InternalError::new(format!("Failed to deserialize token from Discord: {err}"))
        })?;

    let conn = state.database_connection.lock().await;
    let name_maybe = conn.query_row(
        "select * from user where discord_id = (?1)",
        (&user_data.id,),
        |row| {
            Ok(row
                .get::<usize, String>(1)
                .expect("Table 'user' has known layout"))
        },
    );

    let cookie = state
        .store
        .store_session(Session::new())
        .await
        .expect("Storing into memory store should never fail.")
        .expect("Should always get a cookie.");

    // Need to fetch the session out of the store again. It's not possible to
    // just create outside and store a clone, it will lose it's cookie state.
    let mut session = state
        .store
        .load_session(cookie.clone())
        .await
        .expect("We just put this session in.")
        .expect("really!");

    // Note: We reuse the same session cookie name here. So we don't need to
    // reset that cookie.
    let cookie = format!("{SESSION_COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

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
        return Ok((headers, Redirect::to("create_user")).into_response());
    }

    match name_maybe {
        Ok(name) => {
            info!("Logging in existing user");
            // TODO: This should be just fetched from the user table based on the id instead.
            session
                .insert("username", name.clone())
                .expect("Memory store yo!");
            Ok((headers, Redirect::to("/")).into_response())
        }
        Err(err) => Err(InternalError::new(format!(
            "Failed to execute SQL query: {err}"
        ))),
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
            Some((kv.next()?, kv.next()?))
        })
        .find_map(|kv| match kv {
            Some((SESSION_COOKIE_NAME, value)) => Some(value),
            _ => None,
        })?;

    let session = match store.load_session(session_id.to_string()).await {
        Ok(session) => session,
        // TODO(#153): Clear the cookie here
        Err(_) => return None,
    };
    session
}

async fn get_user(
    headers: HeaderMap,
    State(state): State<AuthenticationState>,
) -> Result<Response, InternalError> {
    let session = match get_user_session(&headers, &state.store).await {
        Some(session) => session,
        None => return Ok(StatusCode::UNAUTHORIZED.into_response()),
    };
    let content = DisplayUser {
        username: session.get("discord_username").ok_or_else(|| {
            InternalError::new(format!("Failed to get Discord username from session"))
        })?,
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main("".to_string(), content)
    };
    Ok(Html(content).into_response())
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
    State(state): State<AuthenticationState>,
    Form(user_form): Form<UserForm>,
) -> Result<Response, InternalError> {
    let mut session = match get_user_session(&headers, &state.store).await {
        Some(session) => session,
        None => return Ok(StatusCode::UNAUTHORIZED.into_response()),
    };
    let discord_id: String = session
        .get("discord_id")
        .ok_or_else(|| InternalError::new("No discord_id in session".to_string()))?;

    let username = user_form.username;
    let conn = state.database_connection.lock().await;

    conn.execute(
        "insert into user (username, discord_id) values ((?1), (?2))",
        (&username, &discord_id),
    )
    .map_err(|err| InternalError::new(format!("Failed to insert user in db: {err}")))?;

    session.remove("discord_id");

    info!("New user created");

    session
        .insert("username", username.clone())
        .expect("Session is stored in memory");

    let content = WelcomeUser {
        username: username.clone(),
    }
    .render()
    .expect("Template rendering should always succeed");
    // Always redraw everything to update log in state.
    Ok(Html(render_main(username, content)).into_response())
}

struct InternalError {
    message: String,
}

impl InternalError {
    fn new(message: String) -> InternalError {
        InternalError { message }
    }
}

impl IntoResponse for InternalError {
    fn into_response(self) -> Response {
        // (thak): I find it somewhat dubious to log here in the conversion
        // function ... but I can't deny it's convenient.
        error!(
            "Error encountered while processiong request: {}",
            self.message
        );
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
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
