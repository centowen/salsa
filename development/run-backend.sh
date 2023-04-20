if [ ! -e "database.json" ]; then
    cp -r development/database.json database.json
fi
RUST_LOG=Info cargo run --package backend --bin backend
