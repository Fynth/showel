# Contributing to Showel

Thank you for your interest in contributing to Showel! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Pull Request Process](#pull-request-process)
- [Issue Guidelines](#issue-guidelines)

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inspiring community for all. Please be respectful and considerate in all interactions.

### Expected Behavior

- Be respectful and inclusive
- Accept constructive criticism gracefully
- Focus on what is best for the community
- Show empathy towards other community members

### Unacceptable Behavior

- Harassment or discrimination of any kind
- Trolling, insulting comments, or personal attacks
- Publishing others' private information
- Other conduct which could reasonably be considered inappropriate

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- Git
- PostgreSQL (for testing)
- A text editor or IDE (VS Code, RustRover, etc.)

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/showel.git
   cd showel
   ```
3. Add upstream remote:
   ```bash
   git remote add upstream https://github.com/ORIGINAL_OWNER/showel.git
   ```

## Development Setup

### Build the Project

```bash
# Install dependencies and build
cargo build

# Run the application
cargo run

# Build optimized release version
cargo build --release
```

### Development Tools

Install recommended tools:

```bash
# Code formatting
rustup component add rustfmt

# Linting
rustup component add clippy

# Auto-reload during development
cargo install cargo-watch
```

### Run with Auto-Reload

```bash
cargo watch -x run
```

### Enable Debug Logging

```bash
RUST_LOG=showel=debug cargo run
```

## How to Contribute

### Areas for Contribution

1. **Bug Fixes**: Fix reported issues
2. **New Features**: Implement features from TODO.md
3. **Documentation**: Improve or translate docs
4. **Testing**: Add unit and integration tests
5. **Performance**: Optimize slow operations
6. **UI/UX**: Improve user interface

### Good First Issues

Perfect for newcomers:

- Add keyboard shortcuts (Ctrl+Enter to execute query)
- Implement query history storage
- Add CSV export functionality
- Improve error messages with suggestions
- Write unit tests for database operations
- Add tooltips to UI elements

### Finding Issues

- Check the [Issues](../../issues) page
- Look for `good-first-issue` label
- Check `help-wanted` label for areas needing work
- Review TODO.md for planned features

## Coding Standards

### Rust Style Guide

Follow the official [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/).

### Key Points

1. **Formatting**: Use `cargo fmt` before committing
2. **Linting**: Run `cargo clippy` and fix warnings
3. **Naming**: Follow Rust conventions
   - `snake_case` for functions and variables
   - `CamelCase` for types and traits
   - `SCREAMING_SNAKE_CASE` for constants
4. **Documentation**: Add doc comments for public APIs
5. **Error Handling**: Use `Result<T, E>` and provide context

### Code Example

```rust
/// Connects to a PostgreSQL database with the given configuration.
///
/// # Arguments
///
/// * `config` - Connection parameters including host, port, database, etc.
///
/// # Returns
///
/// Returns `Ok(())` on successful connection, or an error if connection fails.
///
/// # Examples
///
/// ```
/// let config = ConnectionConfig {
///     host: "localhost".to_string(),
///     port: 5432,
///     database: "mydb".to_string(),
///     user: "postgres".to_string(),
///     password: "password".to_string(),
/// };
/// connection.connect(config).await?;
/// ```
pub async fn connect(&self, config: ConnectionConfig) -> Result<()> {
    // Implementation
}
```

### File Organization

```
src/
├── main.rs          # Entry point, minimal logic
├── app.rs           # Application state and update loop
├── db.rs            # Database operations
├── ui.rs            # UI components
└── utils.rs         # Helper functions (if needed)
```

### Module Structure

- Keep files under 500 lines when possible
- Group related functionality
- Use clear, descriptive names
- Minimize public API surface

## Testing Guidelines

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
    }

    #[tokio::test]
    async fn test_database_connection() {
        // Test async database operations
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_connection_config

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_test
```

### Test Coverage

- Aim for >70% code coverage on new code
- Test edge cases and error conditions
- Mock external dependencies when needed

## Pull Request Process

### Before Submitting

1. **Update your fork**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Create a feature branch**:
   ```bash
   git checkout -b feature/my-awesome-feature
   ```

3. **Make your changes**:
   - Write clean, documented code
   - Follow coding standards
   - Add tests if applicable

4. **Test your changes**:
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

5. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add awesome feature"
   ```

### Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(db): add query timeout support

Implements configurable query timeout to prevent long-running
queries from blocking the UI.

Closes #123
```

```
fix(ui): correct table pagination calculation

The page count was incorrect for tables with row counts
not evenly divisible by page size.

Fixes #456
```

### Submitting Pull Request

1. **Push to your fork**:
   ```bash
   git push origin feature/my-awesome-feature
   ```

2. **Create Pull Request** on GitHub:
   - Use a clear, descriptive title
   - Reference related issues
   - Describe what changed and why
   - Include screenshots for UI changes
   - Mark as draft if work in progress

3. **PR Template**:
   ```markdown
   ## Description
   Brief description of changes

   ## Type of Change
   - [ ] Bug fix
   - [ ] New feature
   - [ ] Breaking change
   - [ ] Documentation update

   ## Testing
   How has this been tested?

   ## Checklist
   - [ ] Code follows style guidelines
   - [ ] Self-review completed
   - [ ] Comments added for complex code
   - [ ] Documentation updated
   - [ ] Tests added/updated
   - [ ] All tests passing
   ```

### Review Process

- Maintainers will review your PR
- Address feedback promptly
- Keep discussions professional
- Be patient - reviews take time

### After Approval

- Squash commits if requested
- Maintainer will merge when ready
- Delete your feature branch after merge

## Issue Guidelines

### Reporting Bugs

Use the bug report template:

```markdown
**Description**
Clear description of the bug

**Steps to Reproduce**
1. Go to '...'
2. Click on '...'
3. See error

**Expected Behavior**
What should happen

**Actual Behavior**
What actually happens

**Environment**
- OS: [e.g., Ubuntu 22.04]
- Rust version: [e.g., 1.75]
- PostgreSQL version: [e.g., 15.3]

**Screenshots**
If applicable

**Additional Context**
Any other information
```

### Suggesting Features

Use the feature request template:

```markdown
**Feature Description**
Clear description of the feature

**Use Case**
Why is this feature needed?

**Proposed Solution**
How might this work?

**Alternatives Considered**
Other approaches you've thought about

**Additional Context**
Mockups, examples, etc.
```

### Security Issues

**DO NOT** open public issues for security vulnerabilities.

Instead:
- Email: security@showel.dev (if available)
- Or create a private security advisory on GitHub

## Development Workflow

### Typical Workflow

1. Check issues or TODO.md for tasks
2. Comment on issue to claim it
3. Create feature branch
4. Implement changes with tests
5. Run quality checks
6. Submit pull request
7. Address review feedback
8. Celebrate merge! 🎉

### Communication

- Ask questions in issues/discussions
- Be responsive to feedback
- Share progress updates
- Help other contributors

## Building Documentation

```bash
# Generate API docs
cargo doc --open

# Check documentation
cargo doc --no-deps
```

## Performance Testing

```bash
# Build with optimizations
cargo build --release

# Profile with perf (Linux)
perf record -g target/release/showel
perf report

# Memory profiling with valgrind
valgrind --tool=massif target/release/showel
```

## Resources

### Learning Resources
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [egui Documentation](https://docs.rs/egui/)
- [tokio Tutorial](https://tokio.rs/tokio/tutorial)

### Project Resources
- [README.md](README.md) - Main documentation
- [QUICKSTART.md](QUICKSTART.md) - Getting started
- [TODO.md](TODO.md) - Feature roadmap
- [OVERVIEW.md](OVERVIEW.md) - Architecture details

## Questions?

- Open a [Discussion](../../discussions)
- Check existing issues
- Read the documentation
- Ask in pull request comments

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to Showel! Your efforts help make this project better for everyone. 🚀