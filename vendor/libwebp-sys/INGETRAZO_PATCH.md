# Metadata-only Cargo override support

Upstream crate libwebp-sys 0.14.4 is retained byte-for-byte except for `links = "webpsys"` in Cargo.toml. Its upstream source archive SHA-256 is `6b3a87b44e34d17161e4f17d92a463d596cb13825dcd1758ed18fd3a721e189c`.

The build script already compiles and links webpsys. Declaring that Cargo identity lets explicit Linux-to-Apple Rust metadata checks override the C build script, as with the existing native font dependencies. These checks produce no executable and prove no C ABI, linking, SDK or GPU behavior. Actual Linux, Windows and macOS builds keep the unmodified upstream build script and bundled C implementation. Native macOS CI remains required.
