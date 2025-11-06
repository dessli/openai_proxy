#!/bin/bash

VERSION="0.1.0"
PACKAGE_NAME="openai_proxy-macos-v${VERSION}"

echo "📦 Building release version..."
cargo build --release

echo "📁 Creating package..."
mkdir -p "${PACKAGE_NAME}"

# Copy files
cp target/release/openai_proxy "${PACKAGE_NAME}/"
cp config.toml "${PACKAGE_NAME}/config.toml"

# Create startup script
cat > "${PACKAGE_NAME}/start.sh" << 'EOF'
#!/bin/bash
cd "$(dirname "$0")"
./openai_proxy
EOF

chmod +x "${PACKAGE_NAME}/start.sh"
chmod +x "${PACKAGE_NAME}/openai_proxy"

# Create README
cat > "${PACKAGE_NAME}/README.md" << 'EOF'
# OpenAI Proxy Server

## Quick Start

1. Edit the `config.toml` configuration file
2. Run `./start.sh` to start the server
3. Or directly run `./openai_proxy`

## Configuration

Edit the `config.toml` file