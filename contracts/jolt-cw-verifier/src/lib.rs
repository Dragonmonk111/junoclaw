pub mod contract;
pub mod error;
pub mod msg;
pub mod state;

#[cfg(test)]
mod tests;

// CosmWasm runs in a deterministic environment — no OS RNG available.
// The `getrandom` crate (pulled transitively by jolt-verifier via rand_core)
// normally uses wasm-bindgen's `js` feature for wasm32, but that produces
// `__wbindgen_placeholder__` imports which CosmWasm's VM rejects. With the
// `custom` feature, we register a no-op implementation. Verification is
// deterministic and never calls this; if it did, it would get zero-filled
// buffers (which would cause a proof check to fail, not a panic).
getrandom::register_custom_getrandom!(custom_getrandom);

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    buf.fill(0);
    Ok(())
}
