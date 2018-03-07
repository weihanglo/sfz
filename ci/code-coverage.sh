set -ex


main() {
    cargo install cargo-tarpaulin &&
    cargo tarpaulin --out Xml &&
    bash <(curl -s https://codecov.io/bash) &&
    echo "Uploaded code coverage"
}
if [[ "$TRAVIS_RUST_VERSION" == stable && $TRAVIS_OS_NAME == linux ]]; then
    main
fi
