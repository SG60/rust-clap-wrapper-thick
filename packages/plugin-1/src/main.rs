use nih_plug::prelude::*;

use rust_plugin_1::RustPlugin1;

fn main() {
    nih_export_standalone::<RustPlugin1>();
}
