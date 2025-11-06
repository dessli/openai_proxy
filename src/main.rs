use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::HeaderValue, HeaderName},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::str::FromStr;
use config::Config;
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    openai_api_key: String,
    openai_api_base: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct Settings {
    openai_api_key: String,
    openai_api_base: String,
    server_host: String,
    server_port: u16,
}

impl Settings {
    fn load() -> Result<Self, config::ConfigError> {
        // Load .env file if exists
        dotenv::dotenv().ok();

        let config = Config::builder()
            // Read from config.toml file
            .add_source(config::File::with_name("config").required(false))
            // Read from environment variables (higher priority)
            .add_source(
                config::Environment::with_prefix("APP")
                    .separator("_")
            )
            // Set default values
            .set_default("openai_api_base", "https://api.openai.com")?
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
    println!("   - Server: {}:{}", settings.server_host, settings.server_port);
    println!("   - API Base: {}", settings.openai_api_base);
    println!("   - API Key: {}***", &settings.openai_api_key.chars().take(10).collect::<String>());

    let state = Arc::new(AppState {
        openai_api_key: settings.openai_api_key,
        openai_api_base: settings.openai_api_base,
        client: reqwest::Client::new(),
    });

    // Build router
    let app = Router::new()
        .route("/", get(root))
        .route("/v1/*path", post(proxy_handler))
        .route("/v1/*path", get(proxy_handler))
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
    let path = req.uri().path().trim_start_matches('/');
    let query = req.uri().query().unwrap_or("");

    // Build OpenAI API URL using configured API base
    let openai_url = if query.is_empty() {
        format!("{}/{}", state.openai_api_base.trim_end_matches('/'), path)
    } else {
        format!("{}/{}?{}", state.openai_api_base.trim_end_matches('/'), path, query)
    };

    println!("📤 Proxying request to: {}", openai_url);

    // Get original HTTP method
    let method = req.method().clone();

    // Read request body
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| ProxyError::BodyReadError(e.to_string()))?;

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
    let mut request_builder = state.client
        .request(
            reqwest_method,
            &openai_url,
        )
        .header("Authorization", format!("Bearer {}", state.openai_api_key))
        .header("Content-Type", "application/json");

    // Forward other necessary headers
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        // Skip certain headers that should not be forwarded
        if name_str != "host" 
            && name_str != "authorization" 
            && name_str != "content-length" {
            // Convert Axum header name/value to string representations for Reqwest
            request_builder = request_builder.header(name.as_str(), value.to_str().unwrap_or_default());
        }
    }

    // Add request body
    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.to_vec());
    }

    // Send request
    let response = request_builder
        .send()
        .await
        .map_err(|e| ProxyError::RequestError(e.to_string()))?;

    // Get response status
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // Get response headers
    let mut response_headers = HeaderMap::new();
    for (name, value) in response.headers().iter() {
        if name != "content-length" && name != "transfer-encoding" {
            // Convert reqwest headers to axum headers
            if let Ok(header_name) = HeaderName::from_str(name.as_str()) {
                if let Ok(header_value) = HeaderValue::from_str(value.to_str().unwrap_or_default()) {
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

#[derive(Debug)]
enum ProxyError {
    BodyReadError(String),
    RequestError(String),
    ResponseError(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ProxyError::BodyReadError(e) => (StatusCode::BAD_REQUEST, format!("Failed to read request body: {}", e)),
            ProxyError::RequestError(e) => (StatusCode::BAD_GATEWAY, format!("Failed to send request to OpenAI: {}", e)),
            ProxyError::ResponseError(e) => (StatusCode::BAD_GATEWAY, format!("Failed to read response from OpenAI: {}", e)),
        };

        let body = serde_json::json!({
            "error": {
                "message": message,
                "type": "proxy_error"
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
