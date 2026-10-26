# Install the `silt` command (REPL, script runner, language server) to ~/.cargo/bin
install:
    cargo install --path cli --locked

# Start the REPL from source without installing
repl *args:
    cargo run -p silt-cli -- {{args}}
