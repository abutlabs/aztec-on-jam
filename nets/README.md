# Nets

The service lands on jamswap's tiny nets; nothing here starts a chain of its own.

| Net | Start | RPC |
| --- | --- | --- |
| `lasair6` | `cd ../jamswap && docker compose -f docker-compose.lasair6.yml up -d --build` | JIP-2 on a reader |
| `pj6` (PolkaJam only) | `cd ../jamswap && ./dex up NET=pj6` | JIP-2 on `rpc` |

Deploy: `python3 ../jamswap/offchain/deploy.py --rpc ws://localhost:19800 --chain-spec <spec> --code services/privacy/privacy-service.jam`.
A compose file for the indexer, builder, PXE and wallet joins here at plan step 3.
