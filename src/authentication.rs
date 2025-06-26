use std::{collections::HashMap, fmt::Display, fs::read_to_string};

use askama::Template;
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    Form, Router,
    extract::{Path, Query, Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use log::{debug, info, trace, warn};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl, basic::BasicClient,
};
use reqwest::header::USER_AGENT;
use rusqlite::Result;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::index::render_main;
use crate::user::User;
use crate::{app::AppState, error::InternalError};

const SESSION_COOKIE_NAME: &str = "session";

pub async fn extract_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Authenticating user session");
    let session = get_user_session(request.headers(), &state.store).await;
    trace!("Session get: {}", session.is_some());
    let user = match session.and_then(|s| s.get::<i64>("user_id")) {
        // TODO: InternalError -> Not logged in ... ok?
        Some(user_id) => {
            trace!("Got a user id from session");
            User::fetch(state.database_connection, user_id)
                .await
                .unwrap_or(None)
        }
        None => {
            trace!("No user id in session");
            None
        }
    };
    request.extensions_mut().insert(user.clone());
    if user.is_some() {
        debug!("User authenticated.");
    } else {
        debug!("No user logged in.");
    }
    Ok(next.run(request).await)
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/authorized", get(authenticate_from_oauth2))
        .route("/create_user", get(get_user))
        .route("/create_user", post(post_user))
        .route("/login", get(login))
        .route("/redirect/{provider_name}", get(redirect_to_auth_provider))
        .with_state(state)
}

#[derive(Deserialize)]
struct Secrets {
    auth_provider: HashMap<String, Provider>,
}

#[derive(Deserialize, Clone)]
struct Provider {
    auth_uri: String,
    token_uri: String,
    redirect_uri: String,
    user_uri: String,
    display_name_field: String,
    client_id: String,
    client_secret: String,
}

fn read_auth_providers() -> Result<HashMap<String, Provider>, InternalError> {
    let filename = ".secrets.toml";
    let contents = read_to_string(filename)
        .map_err(|err| InternalError::new(format!("Failed to read from '{filename}': {err}")))?;
    let secrets: Secrets = toml::from_str(&contents).map_err(|err| {
        InternalError::new(format!("Failed to parse toml from {filename}: {err}"))
    })?;
    Ok(secrets.auth_provider)
}

fn read_auth_provider(provider_name: &str) -> Result<Provider, InternalError> {
    let auth_providers = read_auth_providers()?;
    let Some(auth_provider) = auth_providers.get(provider_name) else {
        return Err(InternalError::new(format!(
            "No provider with name {provider_name} configured"
        )));
    };
    Ok(auth_provider.clone())
}

// Basic Oath2 flow
//
// 1. User is prompted to select oauth2 provider.
// 2. User is redirected to OAuth2 provider.
// 3. User comes back with an authorization code.
// 4. We use that code to request an authorization token from oauth2 provider.
// 5. We use the token to get the identity of the user from oauth2 provider.

#[derive(Template)]
#[template(path = "login.html")]
struct SelectAuthProvider {
    provider_names: Vec<String>,
}

// 1. User is prompted to select oauth2 provider.
async fn login() -> Result<impl IntoResponse, InternalError> {
    let auth_providers = read_auth_providers()?;
    let mut provider_names: Vec<_> = auth_providers.keys().cloned().collect();
    provider_names.sort();
    let content = SelectAuthProvider { provider_names }
        .render()
        .expect("Template rendering should always succeed");
    Ok(Html(render_main(None, content)))
}

// 2. We redirect the user to auth provider (e.g. Discord) where they authorize our app.
async fn redirect_to_auth_provider(
    Path(provider): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, InternalError> {
    // To know that we're the originator of the request when the user comes back from OAuth2 provider

    let auth_provider = read_auth_provider(&provider)?;
    let client = BasicClient::new(ClientId::new(auth_provider.client_id.clone()))
        .set_auth_uri(
            AuthUrl::new(auth_provider.auth_uri.clone()).expect("Hardcoded URL should always work"),
        )
        // TODO: This url should be retrieved from where we are deployed.
        .set_redirect_uri(
            RedirectUrl::new(auth_provider.redirect_uri.clone())
                .expect("Hardcoded URL should always work."),
        );
    let (url, token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .add_extra_param("prompt".to_string(), "none".to_string())
        .url();

    let mut session = Session::new();
    session
        .insert("csrf_token", &token)
        .expect("Data created entirely by us");
    session
        .insert("provider", &provider)
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

    info!("Sending user to {provider} to authenticate");

    Ok((headers, Redirect::to(url.as_ref())))
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    // We store the CSRF token in the state.
    state: String,
}

struct CsrfError {}

impl Display for CsrfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CSRF error occured")
    }
}

fn validate_csrf_token(
    session: &Session,
    user_supplied_token: String,
) -> std::result::Result<(), CsrfError> {
    if let Some(session_token) = session.get::<String>("csrf_token") {
        if session_token == user_supplied_token {
            debug!("CSRF token ok");
            Ok(())
        } else {
            debug!("CSRF token doesn't match");
            Err(CsrfError {})
        }
    } else {
        debug!("No CSRF token in session");
        Err(CsrfError {})
    }
}

// 3. User comes back with an authorization code.
async fn authenticate_from_oauth2(
    Query(query): Query<AuthRequest>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, InternalError> {
    debug!("Coming back from OAuth2 provider");
    let session = match get_user_session(&headers, &state.store).await {
        Some(session) => session,
        None => {
            debug!("No session found");
            return Ok(StatusCode::UNAUTHORIZED.into_response());
        }
    };
    if let Err(err) = validate_csrf_token(&session, query.state) {
        debug!("Failed to validate CSRF token from oauth2 provider: {err}");
        // This realistically happens because of an old or bogus request
        // to this endpoint. Returning 401 is reasonable.
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let Some(provider_name) = session.get::<String>("provider") else {
        warn!("No provider set in the session, unabled to use code");
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    };

    // 4. We use that code to request an authorization token from oauth2 provider.
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Hardcoded client should always build.");

    let provider = read_auth_provider(&provider_name)?;
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        // TODO: This url should be retrieved from where we are deployed.
        .set_redirect_uri(
            RedirectUrl::new(provider.redirect_uri.clone())
                .expect("Hardcoded URL should always work."),
        )
        .set_token_uri(
            TokenUrl::new(provider.token_uri.clone()).expect("Hardcoded URL should always work."),
        );
    let token = match client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(&http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            debug!("Failed to get token from {provider_name}: {err}");
            // This realistically happens because of an old or bogus request
            // to this endpoint. Returning 401 is reasonable.
            return Ok(StatusCode::UNAUTHORIZED.into_response());
        }
    };

    debug!("Code authenticated");

    // 5. We use the token to get the identity of the user from oauth2 provider.
    let user_data: Map<String, Value> = http_client
        .get(&provider.user_uri)
        .header(USER_AGENT, "salsa/1.0.0")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|err| {
            InternalError::new(format!("Failed to fetch token from {provider_name}: {err}"))
        })?
        .json::<Map<String, Value>>()
        .await
        .map_err(|err| {
            InternalError::new(format!(
                "Failed to deserialize user from {provider_name}: {err}"
            ))
        })?;

    let user_id = match user_data.get("id") {
        Some(Value::Number(user_id)) => format!("{}", user_id),
        Some(Value::String(user_id)) => user_id.clone(),
        None => {
            return Err(InternalError::new(format!(
                "No id field in user object returned from {}",
                &provider.user_uri
            )));
        }
        _ => {
            return Err(InternalError::new(format!(
                "Id field in user object returned from {} had unexpected type",
                &provider.user_uri
            )));
        }
    };

    let user = User::fetch_with_user_with_external_id(
        state.database_connection,
        provider_name.clone(),
        &user_id,
    )
    .await?;

    let cookie = state
        .store
        .store_session(Session::new())
        .await
        .expect("Storing into memory store should never fail.")
        .expect("Should always get a cookie.");

    // Need to fetch the session out of the store again. It's not possible to
    // just create outside and store a clone, it will lose its cookie state.
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

    match user {
        Some(User { id, .. }) => {
            info!("Logging in existing user");
            session.insert("user_id", id).expect("Memory store yo!");
            Ok((headers, Redirect::to("/")).into_response())
        }
        None => {
            debug!("Redirecting to create new user");
            let Some(username) = user_data
                .get(&provider.display_name_field)
                .map(|username| username.as_str())
            else {
                return Err(InternalError::new(format!(
                    "No {} field in user object returned from {}",
                    &provider.display_name_field, &provider.user_uri
                )));
            };
            session
                .insert("external_id", user_id)
                .expect("MemoryStore should work every time");
            session
                .insert("provider_name", provider_name)
                .expect("MemoryStore should work every time");
            session
                .insert("user_display_name", username)
                .expect("MemoryStore should work every time");
            Ok((headers, Redirect::to("create_user")).into_response())
        }
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

    // TODO(#153): Clear the cookie here on failure to get the session.
    store
        .load_session(session_id.to_string())
        .await
        .unwrap_or(None)
}

async fn get_user(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, InternalError> {
    let session = match get_user_session(&headers, &state.store).await {
        Some(session) => session,
        None => return Ok(StatusCode::UNAUTHORIZED.into_response()),
    };
    let content = DisplayUser {
        username: session.get("user_display_name").ok_or_else(|| {
            InternalError::new("Failed to get user_display_name from session".to_string())
        })?,
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(None, content)
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
    State(state): State<AppState>,
    Form(user_form): Form<UserForm>,
) -> Result<Response, InternalError> {
    let mut session = match get_user_session(&headers, &state.store).await {
        Some(session) => session,
        None => return Ok(StatusCode::UNAUTHORIZED.into_response()),
    };
    let provider_name: String = session
        .get("provider_name")
        .ok_or_else(|| InternalError::new("No provider_name in session".to_string()))?;
    let external_id: String = session
        .get("external_id")
        .ok_or_else(|| InternalError::new("No external_id in session".to_string()))?;

    let username = user_form.username;

    let user = User::create_from_external(
        state.database_connection,
        username.clone(),
        provider_name.clone(),
        &external_id,
    )
    .await?;

    session.remove("provider_name");
    session.remove("external_id");

    info!("New user created");

    session
        .insert("user_id", user.id)
        .expect("Session is stored in memory");

    let content = WelcomeUser {
        username: username.clone(),
    }
    .render()
    .expect("Template rendering should always succeed");
    // Always redraw everything to update log in state.
    Ok(Html(render_main(Some(user), content)).into_response())
}
