# ESMig

A command-line tool for managing Elasticsearch migrations written in Rust. ESMig provides a simple and reliable way to version and apply schema changes to your Elasticsearch cluster.

## Features

- **Migration Management**: Create, run, and revert Elasticsearch migrations
- **Version Tracking**: Automatically tracks applied migrations in a hidden Elasticsearch index
- **Custom DSL**: Simple domain-specific language for defining Elasticsearch operations
- **Environment Configuration**: Supports environment-based configuration via `.env` files
- **Literate Programming**: Source code organized using org-mode for better documentation

## Installation

### Prerequisites

- Rust (latest stable version)
- Access to an Elasticsearch cluster

### From Source

```bash
git clone <repository-url>
cd esmig
cargo build --release
```

The binary will be available at `target/release/esmig`.

## Configuration

Create a `.env` file in your project root with the following variables:

```env
ES_ROOT=http://localhost:9200
ES_USER=your_username
ES_PASS=your_password
CIRCUIT=your_environment_name
```

- `ES_ROOT`: Elasticsearch cluster URL
- `ES_USER`: Elasticsearch username
- `ES_PASS`: Elasticsearch password  
- `CIRCUIT`: Environment identifier (used for index naming)

## Usage

### Initialize Migration System

Before running any migrations, initialize the migration tracking system:

```bash
esmig init
```

This creates a hidden index `.{CIRCUIT}-esmigs` in your Elasticsearch cluster to track migration history.

### Create a New Migration

```bash
esmig create <migration_name>
```

This creates a new migration directory with the format:
```
es-migrations/YYYY-MM-DD-HHMMSS-0000_<migration_name>/
├── up.esm    # Forward migration
└── down.esm  # Rollback migration
```

### Run Migrations

Apply all pending migrations:

```bash
esmig run
```

### Revert Last Migration

Rollback the most recently applied migration:

```bash
esmig revert
```

### Redo Last Migration

Revert and then reapply the last migration:

```bash
esmig redo
```

## Migration File Format

Migration files use a simple DSL with the following syntax:

```
# Comments start with #
METHOD /path/to/elasticsearch/endpoint
{
  "json": "body",
  "for": "the request"
}
```

### Supported Methods

- `PUT`: Create or update resources
- `DELETE`: Remove resources

### Example Migration

**up.esm**:
```
# Create a new index
PUT /my_index
{
  "settings": {
    "number_of_shards": 1,
    "number_of_replicas": 0
  },
  "mappings": {
    "properties": {
      "title": { "type": "text" },
      "created_at": { "type": "date" }
    }
  }
}
```

**down.esm**:
```
# Remove the index
DELETE /my_index
```

### Variable Substitution

Use `{circuit}` in your migration files to reference the `CIRCUIT` environment variable:

```
PUT /{circuit}_my_index
{
  "settings": {
    "number_of_shards": 1
  }
}
```

## Project Structure

```
esmig/
├── src/                    # Generated Rust source code
├── es-migrations/          # Migration files directory
├── main.org               # Literate programming source
├── cargo.org              # Cargo configuration in org-mode
├── Cargo.toml             # Rust package manifest
├── .env                   # Environment configuration
└── README.md              # This file
```

## Development

This project uses literate programming with org-mode. The main source code is in `main.org`, and the Rust code is generated using org-babel tangling.

### Building from Org Files

If you modify `main.org`, you'll need to tangle the code to generate the Rust source:

1. Open `main.org` in Emacs with org-mode
2. Run `org-babel-tangle` (C-c C-v t)
3. Build with `cargo build`

### Dependencies

- **clap**: Command-line argument parsing
- **elasticsearch**: Elasticsearch client
- **nom**: Parser combinators for DSL parsing
- **serde_json**: JSON serialization/deserialization
- **tokio**: Async runtime
- **reqwest**: HTTP client
- **chrono**: Date and time handling
- **dotenv**: Environment variable loading
- **glob**: File pattern matching

## License

This project is licensed under the GNU General Public License v2.0 - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please follow these guidelines:

### Getting Started

1. Fork the repository
2. Clone your fork: `git clone <your-fork-url>`
3. Create a feature branch: `git checkout -b feature/your-feature-name`

### Development Setup

1. Install Rust (latest stable version)
2. Install dependencies: `cargo build`
3. Set up your `.env` file with test Elasticsearch credentials
4. Run tests: `cargo test`

### Making Changes

Since this project uses literate programming with org-mode:

1. **Primary source**: Make changes in `main.org`, not directly in `src/main.rs`
2. **Tangle code**: After editing `main.org`, run `org-babel-tangle` in Emacs to generate Rust source
3. **Test changes**: Run `cargo build` and `cargo test` to verify your changes
4. **Follow org-mode conventions**: 
   - Use 2-space indentation in code blocks
   - Add `:noweb yes` for blocks using `<<...>>` tags
   - Place use declarations in the appropriate declaration blocks

### Code Style

- Follow standard Rust formatting (`cargo fmt`)
- Run clippy for linting (`cargo clippy`)
- Add tests for new functionality
- Update documentation in org-mode comments

### Submitting Changes

1. Ensure all tests pass
2. Update documentation if needed
3. Commit your changes with clear, descriptive messages
4. Push to your fork: `git push origin feature/your-feature-name`
5. Create a pull request with:
   - Clear description of changes
   - Reference to any related issues
   - Test results

### Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Include steps to reproduce for bugs
- Provide your environment details (OS, Rust version, Elasticsearch version)

### License

By contributing, you agree that your contributions will be licensed under the GPL2 license.

## Support

[Add support/contact information here]
