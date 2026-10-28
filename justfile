# Install the `silt` command (REPL, script runner, language server) to ~/.cargo/bin
install:
    cargo install --path cli --locked

# Start the REPL from source without installing
repl *args:
    cargo run -p silt-cli -- {{args}}

# A/B the benches: BASE revision against the working tree (see benchmarks/ab.sh)
ab base="HEAD" filter="":
    benchmarks/ab.sh {{base}} '{{filter}}'

# A/B by instruction count; reliable while the machine is busy
ab-count base="HEAD" filter="":
    COUNT=1 benchmarks/ab.sh {{base}} '{{filter}}'
