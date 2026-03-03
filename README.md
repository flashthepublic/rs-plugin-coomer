# rs-plugin-coomer

WASM plugin for looking up content metadata from coomer.st.

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Test

```bash
cargo build --target wasm32-unknown-unknown --release
cargo test --test lookup_test -- --nocapture
```
