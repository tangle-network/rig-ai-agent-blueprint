# Rig AI Documentation Assistant

An intelligent agent that automatically improves code documentation and style across repositories using Claude 3.5 Sonnet.

## Overview

This project provides a Rust-based service that:

- Analyzes entire code repositories for documentation and style improvements
- Uses language-specific expertise for Rust, TypeScript, Python, Go, and other languages
- Makes intelligent decisions about whether to commit changes directly or suggest them as review comments
- Creates pull requests with detailed explanations of all improvements

## Key Features

- **Smart Processing**: Analyzes code files in parallel while respecting language-specific conventions
- **Adaptive Handling**:
  - Automatically commits clear improvements (e.g., spelling fixes)
  - Creates review suggestions for more substantial changes
  - Handles large files by processing them in chunks
- **GitHub Integration**:
  - Creates feature branches for improvements
  - Generates detailed pull requests
  - Adds review comments using GitHub's suggestion format
- **Language Support**:
  - Rust: Documentation comments, attributes, and idiomatic patterns
  - TypeScript/JavaScript: JSDoc, type definitions, and modern practices
  - Python: Google-style docstrings, type hints, and PEP 8 compliance
  - Go: Package documentation and idiomatic Go patterns
  - General support for other languages

## Usage

1. Set up environment variables:

```bash
ANTHROPIC_API_KEY=your_api_key
GITHUB_TOKEN=your_github_token
```

2. Run the service:

```bash
cargo run
```

3. Submit a job with repository details:

```json
{
  "repo_url": "https://github.com/owner/repo",
  "branch": "main",
  "agent_id": 0
}
```

## Configuration

- `MAX_CONCURRENT_FILES`: Number of files to process concurrently (default: 5)
- `MAX_FILE_SIZE`: Maximum file size to process (default: 1MB)
- `CHUNK_SIZE`: Lines of code to process per chunk (default: 500)

## Architecture

- Uses Tokio for async processing
- Implements parallel file processing with controlled concurrency
- Employs thread-safe containers for collecting suggestions
- Integrates with GitHub's API for pull request and review management

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
