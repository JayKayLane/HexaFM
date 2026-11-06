//! Runs the plugin (with its editor window) without a DAW, for testing.
fn main() {
    nih_plug::wrapper::standalone::nih_export_standalone::<hexa_fm_plugin::HexaFm>();
}
