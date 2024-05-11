# Hellomouse Apps API

A backend for various hellomouse apps written in Rust. Current list:
1. **Hellomouse Board:** A quick note/pinboard for storing tidbits of info


## Getting Started:

First, rename `config.toml.example` to `config.toml` and fill in the arguments. A Postgres DB is required to be setup and accessible, and the user should have the CREATE permission.

To run a debug build:

```
cargo run --bin server
```

### Adding new users:

```
cargo build --bin user --release
./target/release/user help
```

Then run the user executable for a list of commands.

### Tests:

These tests require the env variables `TEST_API_USER` and `TEST_API_PASSWORD` (which store a valid login for the site) (you will need to create a user first manually in the DB in the table `users`) and have the API server running and accessible over localhost.

To install test dependencies:
```
pip3 install -r requirements.txt
```


To run tests:

```
cd tests
python3 -m unittest discover
```

## License

See `LICENSE.md`