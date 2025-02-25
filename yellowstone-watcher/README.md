# Yellowstone Watcher


I didn't have enough time to test it entirely, but here is the general flow:

`cargo build --release`

To generate config file: (optional step)

`./target/release/yellowstone-watcher generate-config`

Start listener/watcher:

`./target/release/yellowstone-watcher start`


keypair.json is gitignored as we are testing it on mainnet.