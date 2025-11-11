use axum::{
    body::Body,
    extract::{Request, State},
    http::{header::HeaderValue, HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use config::Config;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    openai_api_key: String,
    openai_api_base: String,
    client: reqwest::Client,
    available_models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
struct ModelInfo {
    id: String,
    object: String,
    owned_by: String,
    #[serde(default)]
    enable_thinking: bool,
    #[serde(default = "default_reasoning_effort")]
    reasoning_effort: String,
}

#[derive(Debug, Deserialize)]
struct Settings {
    openai_api_key: String,
    openai_api_base: String,
    #[serde(default = "default_api_version")]
    api_version: String,
    server_host: String,
    server_port: u16,
    #[serde(default)]
    available_models: Vec<ModelInfo>,
}

fn default_api_version() -> String {
    "v1".to_string()
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

#[derive(Debug)]
enum ProxyError {
    RequestError(String),
    ResponseError(String),
    BodyReadError(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ProxyError::RequestError(msg) => (StatusCode::BAD_GATEWAY, msg),
            ProxyError::ResponseError(msg) => (StatusCode::BAD_GATEWAY, msg),
            ProxyError::BodyReadError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Body::from(format!("Proxy error: {}", message));
        Response::builder().status(status).body(body).unwrap()
    }
}

impl Settings {
    fn load() -> Result<Self, config::ConfigError> {
        // Load .env file if exists
        dotenv::dotenv().ok();

        let config = Config::builder()
            // Read from config.toml file
            .add_source(config::File::with_name("config").required(false))
            // Read from environment variables (higher priority)
            .add_source(config::Environment::with_prefix("APP").separator("_"))
            // Set default values
            .set_default("openai_api_base", "https://api.openai.com")?
            .set_default("api_version", "v1")?
            .set_default("server_host", "127.0.0.1")?
            .set_default("server_port", 8080)?
            .build()?;

        config.try_deserialize()
    }
}

#[tokio::main]
async fn main() {
    // 加载配置
    let settings = Settings::load().unwrap_or_else(|err| {
        eprintln!("❌ Failed to load configuration: {}", err);
        eprintln!("💡 Please create a config.toml file or set environment variables");
        std::process::exit(1);
    });

    println!("📋 Configuration loaded:");
    println!(
        "   - Server: {}:{}",
        settings.server_host, settings.server_port
    );
    println!("   - API Base: {}", settings.openai_api_base);
    println!("   - API Version: {}", settings.api_version);
    println!(
        "   - API Key: {}***",
        &settings.openai_api_key.chars().take(10).collect::<String>()
    );
    println!(
        "   - Available Models: {} models configured",
        settings.available_models.len()
    );

    let state = Arc::new(AppState {
        openai_api_key: settings.openai_api_key,
        openai_api_base: settings.openai_api_base,
        client: reqwest::Client::new(),
        available_models: settings.available_models,
    });

    // Build router
    let app = Router::new()
        .route("/", get(root))
        .route("/v3/*path", post(proxy_handler))
        .route("/v3/*path", get(proxy_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let bind_addr = format!("{}:{}", settings.server_host, settings.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("❌ Failed to bind to {}: {}", bind_addr, err);
            std::process::exit(1);
        });

    println!("🚀 OpenAI Proxy Server running on http://{}", bind_addr);
    println!("📝 Usage: http://{}/v1/chat/completions", bind_addr);
    println!("🔧 Press Ctrl+C to stop");

    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "OpenAI API Proxy Server is running!"
}

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request,
) -> Result<Response, ProxyError> {
    // Extract path and query before consuming the request
    let path = req.uri().path().trim_start_matches('/').to_string();
    let query = req.uri().query().unwrap_or("").to_string();

    // Build OpenAI API URL using configured API base
    let openai_url = if query.is_empty() {
        format!("{}/{}", state.openai_api_base.trim_end_matches('/'), path)
    } else {
        format!(
            "{}/{}?{}",
            state.openai_api_base.trim_end_matches('/'),
            path,
            query
        )
    };

    println!("📤 Proxying request to: {}", openai_url);

    // Get original HTTP method
    let method = req.method().clone();

    // Read request body
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| ProxyError::BodyReadError(e.to_string()))?;

    // Modify request body to add thinking configuration based on the requested model
    let modified_body = if !body_bytes.is_empty() {
        match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            Ok(mut json) => {
                if json.is_object() {
                    let obj = json.as_object_mut().unwrap();

                    // Extract model name first (immutable borrow)
                    let model_name = obj
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Release immutable borrow before mutating
                    if let Some(model_name) = model_name {
                        // Find model configuration
                        if let Some(model_config) =
                            state.available_models.iter().find(|m| m.id == model_name)
                        {
                            // Add thinking parameters if enabled for this model
                            if model_config.enable_thinking {
                                obj.insert(
                                    "thinking".to_string(),
                                    serde_json::json!({"type": "enabled"}),
                                );
                                obj.insert(
                                    "reasoning_effort".to_string(),
                                    serde_json::Value::String(
                                        model_config.reasoning_effort.clone(),
                                    ),
                                );
                                println!(
                                    "🧠 Applied deep thinking for model {} (effort: {})",
                                    model_name, model_config.reasoning_effort
                                );
                            }
                        }
                    }
                }

                serde_json::to_vec(&json).unwrap_or_else(|_| body_bytes.to_vec())
            }
            Err(_) => body_bytes.to_vec(),
        }
    } else {
        body_bytes.to_vec()
    };

    // Convert Axum's Method to Reqwest's Method
    let reqwest_method = match method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::POST, // Default to POST
    };

    // Build forwarding request
    let mut request_builder = state
        .client
        .request(reqwest_method, &openai_url)
        .header("Authorization", format!("Bearer {}", state.openai_api_key))
        .header("Content-Type", "application/json");

    // Forward other necessary headers
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        // Skip certain headers that should not be forwarded
        if name_str != "host" && name_str != "authorization" && name_str != "content-length" {
            // Convert Axum header name/value to string representations for Reqwest
            request_builder =
                request_builder.header(name.as_str(), value.to_str().unwrap_or_default());
        }
    }

    // Add request body
    if !modified_body.is_empty() {
        request_builder = request_builder.body(modified_body);
    }

    // Send request
    let response = request_builder
        .send()
        .await
        .map_err(|e| ProxyError::RequestError(e.to_string()))?;

    // Get response status
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // Check if this is a /models endpoint and response is 404
    if path.ends_with("/models") && status == StatusCode::NOT_FOUND {
        // println!("⚠️  /models endpoint returned 404, using configured models");
        return Ok(return_configured_models(&state));
    }

    // Get response headers
    let mut response_headers = HeaderMap::new();
    for (name, value) in response.headers().iter() {
        if name != "content-length" && name != "transfer-encoding" {
            // Convert reqwest headers to axum headers
            if let Ok(header_name) = HeaderName::from_str(name.as_str()) {
                if let Ok(header_value) = HeaderValue::from_str(value.to_str().unwrap_or_default())
                {
                    response_headers.insert(header_name, header_value);
                }
            }
        }
    }

    // Get response body
    let response_body = response
        .bytes()
        .await
        .map_err(|e| ProxyError::ResponseError(e.to_string()))?;

    println!("✅ Response status: {}", status);

    // Build response
    let mut resp = Response::new(Body::from(response_body));
    *resp.status_mut() = status;
    *resp.headers_mut() = response_headers;

    Ok(resp)
}

fn return_configured_models(state: &AppState) -> Response {
    let models_response = serde_json::json!({
        "object": "list",
        "data": state.available_models
    });

    let json_body = serde_json::to_string(&models_response).unwrap_or_else(|_| "{}".to_string());

    let mut response = Response::new(Body::from(json_body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );
    response
}
