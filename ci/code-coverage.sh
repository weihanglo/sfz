set -ex


main() {
    cargo install cargo-tarpaulin &&
    cargo tarpaulin --out Xml &&
    bash <(curl -s https://codecov.io/bash) &&
    echo "Uploaded code coverage"
}

# Collect code coverage only on linux with musl
if [[ "$TARGET" == x86_64-unknown-linux-musl ]]; then
    main
fi
