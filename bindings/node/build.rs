// Required by napi-rs: generates the `.node` symbol export glue and
// platform-specific link flags. Without `napi_build::setup()` the
// produced cdylib will not be loadable from Node at runtime.
fn main() {
    napi_build::setup();
}
