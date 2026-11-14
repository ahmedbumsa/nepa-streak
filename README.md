# $NEPA Streak

**Track:** Memecoins With Mechanics · **Network:** NEAR testnet · **Live page:** `https://nepa-streak.vercel.app/`

The joke needs rules. $NEPA is a NEP-141 token with a daily "Up NEPA!" ritual enforced by the contract:

- Each round, every holder can `check_in` once.
- If check-ins reach the **target**, everyone who checked in is minted tokens (`claim`) and the **streak** grows.
- If not, a slice of the **treasury** (the contract's own balance) is burned and the streak resets.
- All parameters are fixed at launch and readable with `get_state`. There is no admin function to change them.

## Try it (do X, see Y)

1. `storage_deposit` with 0.00125 NEAR. See: you are registered.
2. `check_in`. See: `get_state` shows `checkins_today` up by one.
3. After the round ends, `settle` then `claim`. See: your `ft_balance_of` grows if the target was met, or the treasury shrinks if not.

The website reads the live state and generates the exact commands.

## Deploy (testnet)

Run in GitHub Codespaces (a browser terminal works from a phone). Check the current NEAR docs for exact tool versions.

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-near near-cli-rs
cd contract && cargo near build non-reproducible-wasm
near account create-account sponsor-by-faucet-service nepa-YOURNAME.testnet autogenerate-new-keypair save-to-keychain network-config testnet create
near contract deploy nepa-YOURNAME.testnet use-file target/near/nepa_streak.wasm with-init-call new json-args '{"owner_id":"YOURNAME.testnet","total_supply":"100000000000000","treasury_bps":3000,"target":3,"mint_amount":"100000000000","burn_bps":1000,"epoch_secs":300}' prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

`epoch_secs: 300` makes a round 5 minutes so judges can see a full cycle. Use `86400` for real days. Decimals are 8, so `100000000000000` is 1,000,000 NEPA.

Then set `CONTRACT` in `index.html` and enable GitHub Pages.

## Honest limits

- Hackathon prototype on testnet, no financial value, not financial advice.
- The website is read-only; actions use the CLI. Wallet connect is the next step.
- Rounds only close when someone calls `check_in`, `settle` or `claim`.

## License

MIT

Deployed testnet contract: nepa-ahmedbumsa.testnet
