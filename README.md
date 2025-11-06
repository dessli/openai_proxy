# OpenAI Proxy Server

A lightweight and configurable proxy server for OpenAI API built with Rust, Axum, and Tokio.

## Features

- 🚀 High-performance async proxy built with Rust
- 🔄 Supports all OpenAI API endpoints
- 🔐 Configurable API key management
- 🌐 Custom API base URL support (for OpenAI-compatible services)
- ⚙️ Flexible configuration via file or environment variables
- 🔌 CORS enabled for web applications
- 📝 Request/response logging
- 🎯 Preserves original HTTP methods (GET, POST, PUT, DELETE, etc.)

## Prerequisites

- Rust 1.70 or higher
- An OpenAI API key

## Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/dessli/openai_proxy.git
cd openai_proxy

# Build the project
cargo build --release

# The binary will be available at target/release/openai_proxy
```
### Using Pre-built Binary

Download the latest release from the releases page and extract it.

## Configuration

The server can be configured using a `config.toml` file or environment variables.

### Using config.toml

Create a `config.toml` file in the same directory as the executable:
```
toml
# OpenAI API Configuration
openai_api_key = "sk-your-api-key-here"
openai_api_base = "https://api.openai.com"

# Server Configuration
# Listen address, default 127.0.0.1 (localhost only)
# Set to 0.0.0.0 to allow LAN access
server_host = "127.0.0.1"

# Listen port, default 8080
server_port = 8080
```
### Using Environment Variables

You can override configuration using environment variables with the `APP_` prefix:

```bash
export APP_OPENAI_API_KEY="sk-your-api-key-here"
export APP_OPENAI_API_BASE="https://api.openai.com"
export APP_SERVER_HOST="127.0.0.1"
export APP_SERVER_PORT="8080"
```


### Configuration Priority

Environment variables have higher priority than `config.toml` settings.

## Usage

### Start the Server

```shell script
# Using the binary directly
./openai_proxy

# Or using cargo
cargo run --release
```


### Expected Output

```
📋 Configuration loaded:
   - Server: 127.0.0.1:8080
   - API Base: https://api.openai.com
   - API Key: sk-2329ecb***
🚀 OpenAI Proxy Server running on http://127.0.0.1:8080
📝 Usage: http://127.0.0.1:8080/v1/chat/completions
🔧 Press Ctrl+C to stop
```


### Making Requests

Point your OpenAI client to your proxy server instead of the official API:

#### Using cURL

```shell script
# List models
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer your-client-token"

# Chat completion
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-client-token" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```


#### Using Python OpenAI SDK

```textmate
import openai

openai.api_base = "http://localhost:8080/v1"
openai.api_key = "your-client-token"  # Can be any string

response = openai.ChatCompletion.create(
    model="gpt-3.5-turbo",
    messages=[{"role": "user", "content": "Hello!"}]
)
print(response.choices[0].message.content)
```


#### Using Node.js

```javascript
const OpenAI = require('openai');

const openai = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: 'your-client-token', // Can be any string
});

async function main() {
  const completion = await openai.chat.completions.create({
    model: 'gpt-3.5-turbo',
    messages: [{ role: 'user', content: 'Hello!' }],
  });
  console.log(completion.choices[0].message.content);
}

main();
```


## Use Cases

### 1. API Key Management
Centralize your OpenAI API key in one place instead of distributing it to multiple clients.

### 2. Rate Limiting & Monitoring
Add custom middleware to implement rate limiting, usage tracking, or request logging.

### 3. Custom API Endpoints
Use with OpenAI-compatible APIs by changing the `openai_api_base` configuration:

```toml
openai_api_base = "https://your-custom-api.com"
```


### 4. Development & Testing
Run a local proxy for development without exposing your API key in client code.

## Project Structure

```
openai_proxy/
├── src/
│   └── main.rs          # Main application code
├── Cargo.toml           # Rust dependencies and metadata
├── config.toml          # Configuration file
└── README.md           # This file
```


## Dependencies

- **axum** (0.7) - Web framework
- **tokio** (1.0) - Async runtime
- **reqwest** (0.11) - HTTP client
- **serde** (1.0) - Serialization/deserialization
- **tower-http** (0.5) - CORS middleware
- **config** (0.14) - Configuration management
- **dotenv** (0.15) - Environment variable loading

## Logging

The server provides simple console logging for all proxied requests:

```
📤 Proxying request to: https://api.openai.com/v1/chat/completions
✅ Response status: 200 OK
```


## Error Handling

The proxy handles and returns appropriate error messages for:

- **400 Bad Request** - Invalid request body
- **502 Bad Gateway** - Failed to communicate with OpenAI API
- **500 Internal Server Error** - Unexpected errors

Error response format:

```json
{
  "error": {
    "message": "Error description",
    "type": "proxy_error"
  }
}
```


## Security Considerations

⚠️ **Important Security Notes:**

1. **Never commit your `config.toml` with real API keys** to version control
2. Add `config.toml` to `.gitignore`
3. Use environment variables in production
4. If exposing to the internet, implement authentication and rate limiting
5. Consider using HTTPS in production (put behind a reverse proxy like nginx)

## Troubleshooting

### Port Already in Use

```
❌ Failed to bind to 127.0.0.1:8080: Address already in use
```


**Solution:** Change the port in `config.toml` or use environment variable:

```shell script
export APP_SERVER_PORT=8081
```


### 405 Method Not Allowed

This usually means the HTTP method is not supported by the endpoint. The proxy preserves the original HTTP method from your request.

### Connection Refused

Ensure the server is running and you're using the correct host and port.

## Building for Production

### Optimize Binary Size

```shell script
# Build with optimization
cargo build --release

# Strip debug symbols (optional)
strip target/release/openai_proxy
```


### Cross-Compilation

```shell script
# For Linux
cargo build --release --target x86_64-unknown-linux-gnu

# For macOS (ARM)
cargo build --release --target aarch64-apple-darwin

# For Windows
cargo build --release --target x86_64-pc-windows-gnu
```


## macOS Application

You can package this as a macOS `.app` bundle using the provided `create_macos_app.sh` script:

```shell script
chmod +x create_macos_app.sh
./create_macos_app.sh
open "OpenAI Proxy.app"
```


## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License MIT

## Changelog

### v0.1.0 (Initial Release)
- Basic proxy functionality
- Configuration file support
- Environment variable support
- CORS enabled
- HTTP method preservation
- Configurable API base URL

## Support

For issues, questions, or contributions, please open an issue on GitHub.

## Acknowledgments

Built with ❤️ using Rust and the amazing open-source ecosystem.
```
