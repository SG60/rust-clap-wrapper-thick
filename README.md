# An example of building fully self-contained plugins with Rust and the [clap-wrapper](https://github.com/free-audio/clap-wrapper) project.

You will need to have the [cbindgen](https://github.com/mozilla/cbindgen) tool installed to build the project. This is used to generate the C bindings for the Rust code. You can install it with Homebrew on MacOS: `brew install cbindgen`

To build the project, run the following command:

```bash
cmake -B clap-wrapper-cmake/build clap-wrapper-cmake -G Xcode && cmake --build clap-wrapper-cmake/build
```

The resulting plugins will be located in the `clap-wrapper-cmake/build/rust-plugin-1_assets` directory.

**Note: This currently only builds on MacOS, since I haven't tested it on Windows or Linux. The list of frameworks in `CMakeLists.txt` needed by the Rust staticlib at the link stage will need adjusting.**
